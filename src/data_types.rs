use serde::{de, Deserialize, Deserializer};
use std::fmt::{Debug, Display};

pub const PRICE_SCALAR: i64 = 10000;

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct Price(pub i64);

impl Price {
    pub fn make_absolute(&mut self) {
        self.0 = self.0.abs();
    }

    pub fn try_add(&mut self, other: Price) -> bool {
        let Some(val) = self.0.checked_add(other.0) else {
            return false;
        };
        self.0 = val;
        true
    }

    pub fn try_sub(&mut self, other: Price) -> bool {
        let Some(val) = self.0.checked_sub(other.0) else {
            return false;
        };
        self.0 = val;
        true
    }
}

impl Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let integral = self.0 / PRICE_SCALAR;
        let fractional = self.0.abs() % PRICE_SCALAR;
        write!(f, "{}.{}", integral, fractional)
    }
}

#[derive(Debug)]
pub struct Float2PriceError;

/// We want to be conservative converting prices here and reject any
/// over/underflow while converting.
/// * Infinite and NaN values are rejected and result in an Error.
/// * The conversion uses the overall price scalar to provide the appropriate
///   decimal precision (See [`PRICE_SCALAR`]).
/// * subnormal numbers are not handled.
impl TryFrom<f64> for Price {
    type Error = Float2PriceError;

    fn try_from(mut value: f64) -> Result<Self, Self::Error> {
        if value.is_infinite() || value.is_nan() {
            return Err(Float2PriceError);
        }

        value = (value * PRICE_SCALAR as f64).round();
        if value <= i64::MAX as f64 && value >= i64::MIN as f64 {
            Ok(Price(value as i64))
        } else {
            Err(Float2PriceError)
        }
    }
}

impl PartialEq<f64> for Price {
    fn eq(&self, other: &f64) -> bool {
        // this is a bit of a naive implementation because we potentially lose
        // precision.
        let Ok(o): Result<Price, _> = (*other).try_into() else {
            return false;
        };

        &o == self
    }
}

impl<'de> Deserialize<'de> for Price {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let float: f64 = Deserialize::deserialize(deserializer)?;
        Price::try_from(float).map_err(|_| de::Error::custom("Invalid amount"))
    }
}

#[derive(Debug, PartialEq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Deserialize)]
pub struct TransactionEvent {
    #[serde(default)]
    pub amount: Price,
    pub tx: u32,
    pub client_id: u16,
    #[serde(rename = "type")]
    pub ty: TransactionType,
}

impl TryFrom<&str> for TransactionType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "deposit" => Ok(TransactionType::Deposit),
            "withdrawal" => Ok(TransactionType::Withdrawal),
            "dispute" => Ok(TransactionType::Dispute),
            "resolve" => Ok(TransactionType::Resolve),
            "chargeback" => Ok(TransactionType::Chargeback),
            _ => Err(format!("Invalid transaction type: {}", value)),
        }
    }
}

impl<'de> Deserialize<'de> for TransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        TransactionType::try_from(s).map_err(serde::de::Error::custom)
    }
}

impl Debug for TransactionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Event: [{}] {:?}, client={}, amount={}",
            self.tx, self.ty, self.client_id, self.amount,
        )
    }
}

#[derive(Debug, PartialEq)]
pub enum TransactionFlags {
    None,
    Disputed,
    Resolved,
    Chargeback,
}

#[derive(Debug)]
pub enum TransactionError {
    Overflow,
    Duplicate,
    NotFound,
    InvalidDispute,
    InsufficientFunds,
    Locked,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Account {
    pub total: Price,
    pub held: Price,
    pub locked: bool,
}

impl Account {
    pub fn withdraw(&mut self, amount: Price) -> Result<(), TransactionError> {
        if self.locked {
            return Err(TransactionError::Locked);
        }

        if amount > self.available() {
            return Err(TransactionError::InsufficientFunds);
        }

        if !self.total.try_sub(amount) {
            return Err(TransactionError::Overflow);
        }

        Ok(())
    }

    pub fn deposit(&mut self, amount: Price) -> Result<(), TransactionError> {
        if self.locked {
            return Err(TransactionError::Locked);
        }

        if !self.total.try_add(amount) {
            return Err(TransactionError::Overflow);
        }
        Ok(())
    }

    #[inline]
    pub fn available(&self) -> Price {
        let scaled = self.total.0 - self.held.0;
        Price(scaled)
    }
}

#[derive(PartialEq, Debug)]
pub enum DepositOrWithdraw {
    Deposit,
    Withdraw,
}
