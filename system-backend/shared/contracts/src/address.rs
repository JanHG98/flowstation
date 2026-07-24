use std::{fmt, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};

pub const MAX_SSI: u32 = 0x00ff_ffff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressError {
    OutOfRange(u32),
    InvalidDecimal(String),
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(value) => write!(f, "SSI value {value} exceeds 24-bit range"),
            Self::InvalidDecimal(value) => write!(f, "invalid decimal SSI value: {value}"),
        }
    }
}

impl std::error::Error for AddressError {}

impl From<ParseIntError> for AddressError {
    fn from(error: ParseIntError) -> Self {
        Self::InvalidDecimal(error.to_string())
    }
}

macro_rules! ssi_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(u32);

        impl $name {
            pub const MIN: u32 = 0;
            pub const MAX: u32 = MAX_SSI;

            pub const fn get(self) -> u32 {
                self.0
            }

            pub fn new(value: u32) -> Result<Self, AddressError> {
                if value <= Self::MAX {
                    Ok(Self(value))
                } else {
                    Err(AddressError::OutOfRange(value))
                }
            }
        }

        impl TryFrom<u32> for $name {
            type Error = AddressError;

            fn try_from(value: u32) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = AddressError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let parsed = value
                    .parse::<u32>()
                    .map_err(|_| AddressError::InvalidDecimal(value.to_owned()))?;
                Self::new(parsed)
            }
        }
    };
}

ssi_type!(Ssi);
ssi_type!(Issi);
ssi_type!(Gssi);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_full_24_bit_range() {
        assert_eq!(Issi::new(MAX_SSI).unwrap().get(), MAX_SSI);
        assert_eq!(Gssi::new(0).unwrap().get(), 0);
    }

    #[test]
    fn rejects_values_above_24_bits() {
        assert!(matches!(Ssi::new(MAX_SSI + 1), Err(AddressError::OutOfRange(_))));
    }

    #[test]
    fn serde_is_numeric_and_roundtrips() {
        let value = Issi::new(4_010_001).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(encoded, "4010001");
        assert_eq!(serde_json::from_str::<Issi>(&encoded).unwrap(), value);
    }
}
