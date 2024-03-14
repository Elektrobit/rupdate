// SPDX-License-Identifier: MIT
use anyhow::anyhow;
use serde::{de::Error, Deserialize, Serialize, Serializer};
use std::fmt;

#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(debug_assertions, derive(Debug))]
#[repr(u8)]
pub enum Variant {
    A,
    B,
}

impl<'de> Deserialize<'de> for Variant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            match String::deserialize(deserializer)?.as_str() {
                "a" | "A" => Ok(Variant::A),
                "b" | "B" => Ok(Variant::B),
                _ => Err(Error::custom("Invalid variant.")),
            }
        } else {
            Variant::try_from(u8::deserialize(deserializer)?)
                .map_err(|e| Error::custom(e.to_string()))
        }
    }
}

impl Serialize for Variant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            self.to_string().serialize(serializer)
        } else {
            serializer.serialize_u8(u8::from(*self))
        }
    }
}

impl From<Variant> for u8 {
    fn from(value: Variant) -> u8 {
        value as u8
    }
}

impl TryFrom<u8> for Variant {
    type Error = anyhow::Error;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0x00 => Ok(Variant::A),
            0x01 => Ok(Variant::B),
            _ => Err(anyhow!("Invalid variant.")),
        }
    }
}

impl Default for Variant {
    fn default() -> Self {
        Variant::A
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Variant::A => write!(f, "A"),
            Variant::B => write!(f, "B"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test deserialization of partition variant.
    #[test]
    fn test_load_json_variant() {
        let test_json = vec![
            ("\"A\"", Some(Variant::A)),
            ("\"a\"", Some(Variant::A)),
            ("\"B\"", Some(Variant::B)),
            ("\"b\"", Some(Variant::B)),
            ("\"C\"", None),
            ("\"c\"", None),
        ];

        for (json, expected) in test_json {
            let result = serde_json::from_str::<Variant>(json);

            if expected.is_some() {
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), expected.unwrap());
            } else {
                assert!(result.is_err());
            }
        }
    }

    /// Test decoding variant from byte.
    #[test]
    fn test_load_binary_variant() {
        let test_binary = vec![
            (vec![0x00], Some(Variant::A)),
            (vec![0x01], Some(Variant::B)),
            (vec![0x02], None),
        ];

        for (ref binary, expected) in test_binary {
            let result = bincode::deserialize::<Variant>(binary);

            if expected.is_some() {
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), expected.unwrap());
            } else {
                println!(
                    "Expected Result {:?} should be None: {:?}",
                    expected, result
                );
                assert!(result.is_err());
            }
        }
    }

    /// Test deserialization of binary encoded variant.
    #[test]
    fn test_serialize_binary() {
        let serialized = bincode::serialize(&Variant::A);

        assert_eq!(vec![0x00], serialized.unwrap());
    }

    #[test]
    fn test_serialize_json() {
        let serialized = serde_json::to_string(&Variant::A);

        assert_eq!("\"A\"", serialized.unwrap());
    }
}
