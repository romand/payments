use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{self, Display};

use crate::amount::*;
use crate::tx::*;

#[derive(Debug)]
pub enum TxProcessingError {
    AmountOverflow,
    InsufficientFunds,
    DepositNotFound,
    TxAlreadyDisputed,
    TxNotDisputed,
    AccountLocked,
}

pub struct TxProcessor {
    clients: HashMap<ClientID, Client>,
    deposits: HashMap<ClientID, HashMap<TxID, Amount>>,
    disputed: HashSet<TxID>,
}

pub struct ClientSummary {
    pub id: ClientID,
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

impl TxProcessor {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            deposits: HashMap::new(),
            disputed: HashSet::new(),
        }
    }

    pub fn process(&mut self, tx: &Tx) -> Result<(), TxProcessingError> {
        match *tx {
            Tx::Deposit {
                client_id,
                tx_id,
                amount,
            } => {
                self.client(client_id)?.deposit(amount)?;
                if self
                    .deposits
                    .entry(client_id)
                    .or_insert(HashMap::new())
                    .insert(tx_id, amount)
                    .is_some()
                {
                    panic!("duplicate transaction id {:?}", tx_id)
                }
                Ok(())
            }
            Tx::Withdrawal {
                client_id, amount, ..
            } => self.client(client_id)?.withdraw(amount),
            Tx::Dispute { client_id, tx_id } => {
                let amount = self.deposit_amount(client_id, tx_id)?;
                if self.disputed.contains(&tx_id) {
                    Err(TxProcessingError::TxAlreadyDisputed)
                } else {
                    self.client(client_id)?.dispute(amount)?;
                    self.disputed.insert(tx_id);
                    Ok(())
                }
            }
            Tx::Resolve { client_id, tx_id } => {
                let amount = self.deposit_amount(client_id, tx_id)?;
                if self.disputed.remove(&tx_id) {
                    Ok(self.client(client_id)?.resolve(amount))
                } else {
                    Err(TxProcessingError::TxNotDisputed)
                }
            }
            Tx::Chargeback { client_id, tx_id } => {
                let amount = self.deposit_amount(client_id, tx_id)?;
                if self.disputed.remove(&tx_id) {
                    Ok(self.client(client_id)?.chargeback(amount))
                } else {
                    Err(TxProcessingError::TxNotDisputed)
                }
            }
        }
    }

    pub fn client_summaries<'a>(
        &'a self,
    ) -> impl Iterator<Item = ClientSummary> + 'a {
        self.clients
            .iter()
            .map(|(client_id, client)| ClientSummary {
                id: *client_id,
                available: client.available,
                held: client.held,
                total: client.total(),
                locked: client.locked,
            })
    }

    fn deposit_amount(
        &self,
        client_id: ClientID,
        tx_id: TxID,
    ) -> Result<Amount, TxProcessingError> {
        let client_deposits = self
            .deposits
            .get(&client_id)
            .ok_or(TxProcessingError::DepositNotFound)?;
        client_deposits
            .get(&tx_id)
            .map(|x| x.clone())
            .ok_or(TxProcessingError::DepositNotFound)
    }

    fn client(
        &mut self,
        client_id: ClientID,
    ) -> Result<&mut Client, TxProcessingError> {
        let client = self.clients.entry(client_id).or_insert(Client::new());
        if client.locked {
            Err(TxProcessingError::AccountLocked)
        } else {
            Ok(client)
        }
    }
}

#[derive(Debug)]
struct Client {
    available: Amount,
    held: Amount,
    locked: bool,
}

// invariant: total == available + held
// invariant: total should be representable as Amount
impl Client {
    fn new() -> Self {
        Self {
            available: Amount::new(),
            held: Amount::new(),
            locked: false,
        }
    }

    fn total(&self) -> Amount {
        self.available
            .checked_add(self.held)
            .expect("invariant violated: total is too big")
    }

    fn deposit(&mut self, amount: Amount) -> Result<(), TxProcessingError> {
        if self.total().checked_add(amount).is_some() {
            Ok(self.available = self
                .available
                .checked_add(amount)
                .expect("invariant violated: total < available"))
        } else {
            Err(TxProcessingError::AmountOverflow)
        }
    }

    fn withdraw(&mut self, amount: Amount) -> Result<(), TxProcessingError> {
        match self.available.checked_sub(amount) {
            Some(x) => Ok(self.available = x),
            None => Err(TxProcessingError::InsufficientFunds),
        }
    }

    fn dispute(&mut self, amount: Amount) -> Result<(), TxProcessingError> {
        match self.available.checked_sub(amount) {
            Some(x) => {
                self.held = self
                    .held
                    .checked_add(amount)
                    .expect("invariant violated: total is too big");
                Ok(self.available = x)
            }
            None => Err(TxProcessingError::InsufficientFunds),
        }
    }

    fn resolve(&mut self, amount: Amount) {
        self.available = self
            .available
            .checked_add(amount)
            .expect("invariant violated: total is too big");
        self.held = self
            .held
            .checked_sub(amount)
            .expect("not enough money is held");
    }

    fn chargeback(&mut self, amount: Amount) {
        self.held = self
            .held
            .checked_sub(amount)
            .expect("not enough money is held");
        self.locked = true
    }
}

impl Display for TxProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::AmountOverflow => write!(f, "amount overflow"),
            Self::InsufficientFunds => write!(f, "insufficient funds"),
            Self::DepositNotFound => write!(f, "deposit not found"),
            Self::TxAlreadyDisputed => {
                write!(f, "transaction is already disputed")
            }
            Self::TxNotDisputed => write!(f, "transaction is not disputed"),
            Self::AccountLocked => write!(f, "account is locked"),
        }
    }
}
impl Error for TxProcessingError {}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{Arbitrary, Gen};

    #[derive(Debug, Clone)]
    struct Txs(Vec<Tx>);

    impl Arbitrary for Txs {
        fn arbitrary(g: &mut Gen) -> Txs {
            let size = usize::arbitrary(g) % g.size();
            let mut txs: Vec<Tx> = vec![Tx::Deposit {
                client_id: 1.into(),
                tx_id: 0.into(),
                amount: Amount::arbitrary(g),
            }];
            let mut next_deposit: u32 = 1;
            let mut next_withdrawal: u32 = size as u32;
            let gen_tx_id =
                |g: &mut Gen, n: u32| (u32::arbitrary(g) % n).into();
            for _ in 1..size {
                txs.push(match u32::arbitrary(g) % 41 {
                    0..=9 => {
                        let tx = Tx::Deposit {
                            client_id: 1.into(),
                            tx_id: next_deposit.into(),
                            amount: Amount::arbitrary(g),
                        };
                        next_deposit += 1;
                        tx
                    }
                    10..=19 => {
                        let tx = Tx::Withdrawal {
                            client_id: 1.into(),
                            tx_id: next_withdrawal.into(),
                            amount: Amount::arbitrary(g),
                        };
                        next_withdrawal += 1;
                        tx
                    }
                    20..=29 => Tx::Dispute {
                        client_id: 1.into(),
                        tx_id: gen_tx_id(g, next_deposit),
                    },
                    30..=39 => Tx::Resolve {
                        client_id: 1.into(),
                        tx_id: gen_tx_id(g, next_deposit),
                    },
                    40 => Tx::Chargeback {
                        client_id: 1.into(),
                        tx_id: gen_tx_id(g, next_deposit),
                    },
                    _ => unreachable!(),
                })
            }
            println!("{:?}", txs);
            Txs(txs)
        }
    }

    quickcheck! {
        fn prop_amounts_are_correct(txs: Txs) -> bool {
            let mut available = Amount::new();
            let mut held = Amount::new();
            let mut total = Amount::new();
            let mut locked = false;

            let mut tx_proc = TxProcessor::new();
            let mut deposit_amounts: HashMap<TxID, Amount> = HashMap::new();
            let Txs(txs) = txs;
            for tx in txs {
                if tx_proc.process(&tx).is_ok() {
                    assert!(!locked);
                    match tx {
                        Tx::Deposit{tx_id, amount, ..} => {
                            deposit_amounts.insert(tx_id, amount);
                            available = available.checked_add(amount).unwrap();
                            total = total.checked_add(amount).unwrap()
                        }
                        Tx::Withdrawal{amount, ..} => {
                            available = available.checked_sub(amount).unwrap();
                            total = total.checked_sub(amount).unwrap()
                        }
                        Tx::Dispute{tx_id, ..} => {
                            let amount = *deposit_amounts.get(&tx_id).unwrap();
                            available = available.checked_sub(amount).unwrap();
                            held = held.checked_add(amount).unwrap()
                        }
                        Tx::Resolve{tx_id, ..} => {
                            let amount = *deposit_amounts.get(&tx_id).unwrap();
                            available = available.checked_add(amount).unwrap();
                            held = held.checked_sub(amount).unwrap()
                        }
                        Tx::Chargeback{tx_id, ..} => {
                            let amount = *deposit_amounts.get(&tx_id).unwrap();
                            held = held.checked_sub(amount).unwrap();
                            total = total.checked_sub(amount).unwrap();
                            locked = true
                        }
                    }
                }
            }

            let s = tx_proc.client_summaries().nth(0).unwrap();
            s.available == available && s.held == held &&
                s.total == total && s.locked == locked
        }
    }
}
