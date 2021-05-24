#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use serde::{Serialize, Serializer};
use std::error::Error;
use std::fmt::{self, Display};
use std::num::ParseIntError;
use std::str::FromStr;

// to avoid floating point arithmetics we represent amounts as int
// number of «minimal representable amount»s — 0.0001
type Money = u64;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Amount(Money);

impl Amount {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn checked_add(self, v: Self) -> Option<Self> {
        let Amount(x) = self;
        let Amount(y) = v;
        x.checked_add(y).map(Amount)
    }

    pub fn checked_sub(self, v: Self) -> Option<Self> {
        let Amount(x) = self;
        let Amount(y) = v;
        x.checked_sub(y).map(Amount)
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseAmountError {
    Parse(ParseIntError),
    TooLarge,
    MultipleDots,
    TooPrecise,
}

impl From<ParseIntError> for ParseAmountError {
    fn from(err: ParseIntError) -> Self {
        Self::Parse(err)
    }
}

impl Display for ParseAmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Parse(ref perr) => write!(f, "int parsing error: {}", perr),
            Self::TooLarge => write!(f, "number is too large"),
            Self::MultipleDots => write!(f, "wrong format: multiple dots"),
            Self::TooPrecise => write!(f, "unsupported precision of >4"),
        }
    }
}

impl Error for ParseAmountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            Self::Parse(ref e) => Some(e),
            _ => None,
        }
    }
}

impl FromStr for Amount {
    type Err = ParseAmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split('.').collect::<Vec<&str>>().as_slice() {
            [ips] => {
                let x: Money = ips.parse()?;
                Ok(Amount(x * 10000))
            }
            [ips, fps] => {
                let ip: Money = if ips.is_empty() { 0 } else { ips.parse()? };

                let fps = fps.trim_end_matches('0');
                if fps.len() > 4 {
                    return Err(Self::Err::TooPrecise);
                }
                let mut fp: Money =
                    if fps.is_empty() { 0 } else { fps.parse()? };
                if fps.len() < 4 {
                    let pad = 4 - fps.len();
                    fp *= 10_u64.pow(pad as u32);
                }

                match ip.checked_mul(10000) {
                    Some(x) => match x.checked_add(fp) {
                        Some(res) => Ok(Amount(res)),
                        None => Err(Self::Err::TooLarge),
                    },
                    None => Err(Self::Err::TooLarge),
                }
            }
            _ => Err(Self::Err::MultipleDots),
        }
    }
}

impl Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(x) = self;
        let mut fp: Money = x % 10000;
        if fp > 0 {
            let mut width = 4;
            while fp % 10 == 0 {
                fp /= 10;
                width -= 1
            }
            write!(f, "{}.{:0width$}", x / 10000, fp, width = width)
        } else {
            write!(f, "{}", x / 10000)
        }
    }
}

impl Serialize for Amount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deser() {
        fn d(s: &str) -> u64 {
            let Amount(x) = s.parse().unwrap();
            x
        }
        assert_eq!(d("1"), 10000);
        assert_eq!(d(".1"), 1000);
        assert_eq!(d("123.4567"), 1234567);
        assert_eq!(d(".067"), 670);
        assert_eq!(d(".0670000"), 670);
        assert_eq!(d(".060"), 600);
        assert_eq!(d("010.0010"), 100010);
        assert_eq!(d("100000000000"), 100_000_000_000_0000);
        assert_eq!(d("+1."), 10000)
    }

    #[test]
    fn test_deser_err() {
        use super::ParseAmountError;
        type E = ParseAmountError;
        fn d(s: &str) -> E {
            if let Err(e) = s.parse::<Amount>() {
                e
            } else {
                panic!("should get error")
            }
        }

        assert!("1e1".parse::<Amount>().is_err());
        assert!("-1".parse::<Amount>().is_err());
        assert!("-0".parse::<Amount>().is_err());
        assert_eq!(d("1844674407370955.1616"), E::TooLarge);
        assert_eq!(d(".12."), E::MultipleDots);
        assert_eq!(d(".01234"), E::TooPrecise);
    }

    #[test]
    fn test_ser() {
        fn s(x: u64) -> String {
            format!("{}", Amount(x))
        }
        assert_eq!(s(0), "0");
        assert_eq!(s(12300), "1.23");
        assert_eq!(s(1234), "0.1234");
        assert_eq!(s(100_000_000_000_0001), "100000000000.0001");
        assert_eq!(s(100_000_000_000_0000), "100000000000")
    }

    impl Arbitrary for Amount {
        fn arbitrary(g: &mut Gen) -> Amount {
            Amount(Money::arbitrary(g))
        }
    }

    quickcheck! {
        fn prop_amount_ser_reversible(amount: Amount) -> bool {
            let s = format!("{}", amount);
            if let Ok(x) = s.parse() {
                amount == x
            } else {
                false
            }
        }
    }
}
