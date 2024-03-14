// SPDX-License-Identifier: MIT
use anyhow::Result;
use ring::digest;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Return a binary representation of the object.
///
/// Tries to generate a binary representation of the object.
///
/// # Error
///
/// If binary serialization fails, an error is returned.
pub trait Hashable {
    // Returns a binary representation of the object.
    fn raw(&self) -> Result<Vec<u8>>;
}

/// Hash algorithm type
///
/// The hash algorithm is an enum representation of the used
/// hash sum algorithm. This enum has to be held in sync with the
/// definition of HashSum. This is important as the hash algorithm
/// defined in the partition configuration directly maps to the
/// used hash sum in the update state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum HashAlgorithm {
    Sha256,
}

impl Default for HashAlgorithm {
    fn default() -> Self {
        HashAlgorithm::Sha256
    }
}

/// Hash sum type
///
/// The hash sum is an enum representation of the used
/// hash sum that holds the actual hash sum as byte array.
/// This enum has to be held in sync with the definition of HashAlgorithm.
/// This is important as the hash algorithm defined in the partition
/// configuration directly maps to the used hash sum in the update state.
#[serde_as]
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub enum HashSum {
    Sha256(#[serde_as(as = "[_; 32]")] [u8; 32]),
}

impl Default for HashSum {
    fn default() -> HashSum {
        unsafe { std::mem::zeroed() }
    }
}

/// Generate a zeroed HashSum for the given HashAlgorithm
impl From<HashAlgorithm> for HashSum {
    fn from(other: HashAlgorithm) -> HashSum {
        match other {
            HashAlgorithm::Sha256 => HashSum::Sha256([0; 32]),
        }
    }
}

impl HashSum {
    /// Construct a new HashSum object based on the given HashAlgorithm over the given data slice
    pub fn generate(bytes: &[u8], algorithm: HashAlgorithm) -> Result<Self> {
        Ok(match algorithm {
            HashAlgorithm::Sha256 => {
                HashSum::Sha256(digest::digest(&digest::SHA256, bytes).as_ref().try_into()?)
            }
        })
    }

    /// Return the corresponding HashAlgorithm
    pub fn algorithm(&self) -> HashAlgorithm {
        match *self {
            HashSum::Sha256(_) => HashAlgorithm::Sha256,
        }
    }

    /// Update the HashSum content based on the new slice data
    pub fn update(&mut self, bytes: &[u8]) -> Result<()> {
        *self = match *self {
            HashSum::Sha256(_) => HashSum::generate(bytes, HashAlgorithm::Sha256)?,
        };

        Ok(())
    }

    /// Return the size of the hash
    pub fn size(&self) -> usize {
        match self {
            Self::Sha256(data) => data.len(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::HashSum;

    use bincode::Options;

    /// Test serialization of a hash sum.
    #[test]
    fn test_serialize_hash_sum() {
        #[rustfmt::skip]
        let checksum: [u8; 32] = [
            0xde, 0x77, 0xa1, 0x9b, 0x1d, 0xd6, 0x45, 0x1c,
            0x34, 0xf7, 0x4a, 0x40, 0xfa, 0x9a, 0xa8, 0x83,
            0xef, 0x9a, 0xdc, 0xe9, 0x39, 0x00, 0xb3, 0x76,
            0x1c, 0xee, 0x8e, 0xe8, 0x4c, 0x0f, 0x0a, 0xea,
        ];

        let hash_sum = HashSum::Sha256(checksum);

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&hash_sum)
            .unwrap();

        let mut expected: [u8; 36] = [0u8; 36];
        expected[4..].copy_from_slice(&checksum);

        assert_eq!(serialized.as_slice(), &expected);
    }
}
