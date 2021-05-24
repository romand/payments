use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

use crate::amount::*;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Copy, Clone)]
pub struct TxID(u32);
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Copy, Clone)]
pub struct ClientID(u16);

#[derive(Debug, Copy, Clone)]
pub enum Tx {
    Deposit {
        client_id: ClientID,
        tx_id: TxID,
        amount: Amount,
    },
    Withdrawal {
        client_id: ClientID,
        tx_id: TxID,
        amount: Amount,
    },
    Dispute {
        client_id: ClientID,
        tx_id: TxID,
    },
    Resolve {
        client_id: ClientID,
        tx_id: TxID,
    },
    Chargeback {
        client_id: ClientID,
        tx_id: TxID,
    },
}

// couldn't make tagged enum (de)serialization work with CSV,
// so we'll read `TxRow`s and later convert them to `Tx`s

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum TxKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Serialize, Deserialize, Debug)]
struct TxRow {
    #[serde(rename = "type")]
    kind: TxKind,
    #[serde(rename = "client")]
    client_id: ClientID,
    #[serde(rename = "tx")]
    tx_id: TxID,
    amount: String,
}

impl Tx {
    fn from_row(tx_row: TxRow) -> Result<Self, ParseAmountError> {
        match tx_row {
            TxRow {
                kind: TxKind::Deposit,
                client_id,
                tx_id,
                amount,
            } => {
                let amount: Amount = amount.parse()?;
                Ok(Tx::Deposit {
                    client_id,
                    tx_id,
                    amount,
                })
            }
            TxRow {
                kind: TxKind::Withdrawal,
                client_id,
                tx_id,
                amount,
            } => {
                let amount: Amount = amount.parse()?;
                Ok(Tx::Withdrawal {
                    client_id,
                    tx_id,
                    amount,
                })
            }
            TxRow {
                kind: TxKind::Dispute,
                client_id,
                tx_id,
                ..
            } => Ok(Tx::Dispute { client_id, tx_id }),
            TxRow {
                kind: TxKind::Resolve,
                client_id,
                tx_id,
                ..
            } => Ok(Tx::Resolve { client_id, tx_id }),
            TxRow {
                kind: TxKind::Chargeback,
                client_id,
                tx_id,
                ..
            } => Ok(Tx::Chargeback { client_id, tx_id }),
        }
    }
}

impl<'de> Deserialize<'de> for Tx {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let row: TxRow = Deserialize::deserialize(deserializer)?;
        Self::from_row(row).map_err(de::Error::custom)
    }
}

#[cfg(test)]
impl From<u16> for ClientID {
    fn from(x: u16) -> Self {
        Self(x)
    }
}

#[cfg(test)]
impl From<u32> for TxID {
    fn from(x: u32) -> Self {
        Self(x)
    }
}
