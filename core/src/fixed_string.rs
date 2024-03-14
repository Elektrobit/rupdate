// SPDX-License-Identifier: MIT
use std::str::FromStr;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Fixed length string type.
///
/// The fixed length string type is a byte sized character array
/// of length SIZE, which is not terminated by a special character.
#[serde_as]
#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FixedString<const SIZE: usize>(#[serde_as(as = "[_; SIZE]")] [u8; SIZE]);

/// Determines the equality of a string slice and a FixedString object.
impl<const SIZE: usize> std::cmp::PartialEq<&str> for FixedString<SIZE> {
    /// Returns true if length and characters in array are equal, false otherwise.
    fn eq(&self, other: &&str) -> bool {
        if other.len() > SIZE {
            return false;
        }

        let other_str: FixedString<SIZE> = if let Ok(inner) = (*other).parse() {
            inner
        } else {
            return false;
        };

        other_str == *self
    }
}

/// Default constructor for FixedString objects.
impl<const SIZE: usize> Default for FixedString<SIZE> {
    /// Initializes a FixedString object with zero bytes.
    fn default() -> FixedString<SIZE> {
        unsafe { std::mem::zeroed() }
    }
}

/// Construct a FixedString object from a string slice.
impl<const SIZE: usize> FromStr for FixedString<SIZE> {
    type Err = anyhow::Error;

    /// Tries to construct a FixedString object from a string slice.
    ///
    /// Returns a new FixedString object containing the slice data,
    /// if the length of the slice is less or equal to the size of
    /// the FixedString object.
    ///
    /// # Error
    ///
    /// If the slice is too large, an error is returned.
    fn from_str(str: &str) -> Result<Self> {
        if str.len() > SIZE {
            return Err(anyhow!(
                "Invalid length {} of fixed string (should be {}).",
                str.len(),
                SIZE
            ));
        }

        let mut fixed_str = Self([0u8; SIZE]);
        fixed_str.0[..str.len()].copy_from_slice(str.as_bytes());

        Ok(fixed_str)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use bincode::Options;

    /// Test the initialization of a FixedString from a string slice.
    #[test]
    fn test_from_str() {
        assert!(FixedString::<36>::from_str("").is_ok());
        assert!(FixedString::<5>::from_str("Hello World").is_err());
        assert_eq!(
            FixedString::<36>::default(),
            FixedString::<36>::from_str("").unwrap()
        );
        assert_eq!(
            FixedString::<36>::from_str("Hello World").unwrap(),
            "Hello World"
        );
    }

    /// Test the comparison of FixedStrings and rust strings.
    #[test]
    fn test_str_cmp() {
        assert!(FixedString::<36>::from_str("").is_ok());
        assert!(FixedString::<5>::from_str("Hello World").is_err());
        assert_eq!(FixedString::<36>::default(), "");
        assert_eq!(FixedString::<36>::from_str("Hello").unwrap(), "Hello");
        assert_eq!(
            FixedString::<36>::from_str("Hello World").unwrap(),
            "Hello World"
        );
        assert_ne!(
            FixedString::<11>::from_str("Hello World").unwrap(),
            "Hello Worlds"
        );
        assert_ne!(FixedString::<36>::default(), "Hello")
    }

    /// Test the default initialization of FixedString.
    #[test]
    fn test_fixed_string_default() {
        let default_str = FixedString::<36>::default();
        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&default_str)
            .unwrap();

        let expected = [0u8; 36];

        assert_eq!(serialized.as_slice(), &expected);
    }

    /// Test the serialization of FixedStrings.
    #[test]
    fn test_serialize_fixed_string() {
        let str = FixedString::<36>::from_str("Hello World").unwrap();

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&str)
            .unwrap();

        let mut expected = [0u8; std::mem::size_of::<FixedString<36>>()];
        expected[..11].copy_from_slice(&[
            b'H', b'e', b'l', b'l', b'o', b' ', b'W', b'o', b'r', b'l', b'd',
        ]);

        assert_eq!(serialized.as_slice(), &expected);
    }
}
