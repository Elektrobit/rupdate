// SPDX-License-Identifier: MIT
use crate::{hash_sum::HashAlgorithm, variant::Variant};
use anyhow::{Context, Result};
#[allow(unused_imports)]
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{collections::HashMap, fmt, fs::File, io::BufReader, path::Path, result};

/// Update environment filesystem name
pub static UPDATE_ENV_FILESYSTEM: &str = "update_fs";
/// Update environment partition set name
pub static UPDATE_ENV_SET: &str = "update_env";

/// Optional partition flags.
#[derive(Clone, Deserialize)]
#[cfg_attr(debug_assertions, derive(Debug, PartialEq, Serialize))]
pub enum PartitionFlags {
    #[serde(alias = "crypto_meta", alias = "CRYPTO_META")]
    CryptoMeta,
    #[serde(alias = "auto_detect", alias = "AUTO_DETECT")]
    AutoDetect,
    #[serde(alias = "part_meta", alias = "PART_META")]
    PartMeta,
    #[serde(alias = "overlay", alias = "OVERLAY")]
    Overlay,
    #[serde(alias = "raw", alias = "RAW")]
    Raw,
}

/// Partition types.
///
/// There are currently two partition types differentiating between formatted
/// and raw partitions.
#[derive(Clone, Deserialize)]
#[cfg_attr(debug_assertions, derive(Debug, PartialEq, Serialize))]
#[serde(untagged)]
pub enum Partitioned {
    /// Unformatted partitions
    RawPartition {
        /// Device name within the linux system or bootloader
        device: String,
        /// Offset within the device (used for unpartitioned space)
        #[serde(deserialize_with = "deserialize_hex_u64")]
        #[cfg_attr(debug_assertions, serde(serialize_with = "serialize_hex_u64"))]
        offset: u64,
    },
    /// Formatted partitions
    FormatPartition {
        /// Device name within the linux system or bootloader
        device: String,
        /// Partition identifier
        partition: String,
    },
}

impl std::fmt::Display for Partitioned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Partitioned::FormatPartition { device, partition } => {
                write!(f, "/dev/{}{}", device, partition)
            }
            Partitioned::RawPartition { device, offset } => {
                write!(f, "/dev/{}@{}", device, offset)
            }
        }
    }
}

/// Deserialize image offsets given in hex format.
///
/// # Error
///
/// If parsing fails an error variant is returned.
fn deserialize_hex_u64<'de, D>(deserializer: D) -> result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct HexStrVisitor;

    impl<'de> Visitor<'de> for HexStrVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a 64bit unsigned int value as decimal or hex digits")
        }

        fn visit_str<E>(self, s: &str) -> result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            if s.starts_with("0x") {
                let without_prefix = s.trim_start_matches("0x");

                u64::from_str_radix(without_prefix, 16)
                    .map_err(|err| de::Error::custom(format!("expected 64bit hex digit: {}", err)))
            } else {
                s.parse::<u64>()
                    .map_err(|err| de::Error::custom(format!("expected 64bit digit: {}", err)))
            }
        }
    }

    let visitor = HexStrVisitor;
    deserializer.deserialize_str(visitor)
}

#[cfg(debug_assertions)]
fn serialize_hex_u64<S>(v: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = format!("{:x}", v);
    serializer.serialize_str(&s)
}

/// Partition description for the linux system and the bootloader.
///
/// The partition description includes all data needed to handle this partition during
/// the boot process and system updates. This includes the partition description
/// for both systems as well as a variant, which distinguishes between the A and B variant of a partition set.
#[derive(Clone, Deserialize)]
#[cfg_attr(debug_assertions, derive(Debug, Default, PartialEq, Serialize))]
pub struct Partition {
    /// Optional variant of the partition (A or B)
    pub variant: Option<Variant>,
    /// Optional description of the partition for linux
    pub linux: Option<Partitioned>,
    /// Optional description of the partition for the bootloader
    pub bootloader: Option<Partitioned>,
}

impl Partition {
    /// Short hand to check if the variant optional is not None.
    pub fn has_variant(&self) -> bool {
        self.variant.is_some()
    }
}

/// Partition Set Description.
///
/// A partition set is the combination of two partitions, which could be
/// swapped out during an update in order to
#[derive(Clone, Deserialize)]
#[cfg_attr(debug_assertions, derive(Debug, Default, PartialEq, Serialize))]
pub struct PartitionSet {
    /// Unique ID of the parition set (legacy)
    pub id: Option<u32>,
    /// Name of the partition set (eg. rootfs, bootfs)
    pub name: String,
    /// Filesystem type
    pub filesystem: Option<String>,
    /// Mountpoint within the linux system
    pub mountpoint: Option<String>,
    /// User defined comment
    #[serde(default)]
    pub comment: String,
    /// List of all partitions
    pub partitions: Vec<Partition>,
    /// List of key/value pairs of user data
    #[serde(default)]
    pub user_data: HashMap<String, String>,
    /// Partition related flags
    #[serde(default)]
    pub flags: Vec<PartitionFlags>,
}

/// Partition configuration.
///
/// The partition configuration includes all data needed by the linux system and
/// the update tool to handle the boot process and system updates. This includes the
#[derive(Deserialize)]
#[cfg_attr(debug_assertions, derive(Debug, Default, PartialEq, Serialize))]
pub struct PartitionConfig {
    /// Version string (eg. 0.1.3)
    pub version: String,
    /// Used hash algorithm for the partition environment (see part_env.rs)
    pub hash_algorithm: HashAlgorithm,
    /// List of partition sets
    pub partition_sets: Vec<PartitionSet>,
}

impl PartitionConfig {
    /// Create a new partition configuration
    ///
    /// Creates and returns a new partition configuration
    /// by parsing the given json file.
    ///
    /// # Error
    ///
    /// Returns an error variant if reading or parsing of the specified
    /// file fails.
    pub fn new<P: AsRef<Path>>(config: P) -> Result<Self> {
        let file = File::open(config.as_ref())?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).with_context(|| {
            format!(
                "Failed to deserialize partition config from {}.",
                config.as_ref().display()
            )
        })
    }

    /// Find a partition set by name.
    pub fn find_set<T: AsRef<str>>(&self, name: T) -> Option<&PartitionSet> {
        self.partition_sets
            .iter()
            .find(|&set| set.name == name.as_ref())
    }

    /// Find the partition set for the update environment.
    pub fn find_update_fs(&self) -> Option<&PartitionSet> {
        self.find_set(UPDATE_ENV_SET)
    }

    /// Find the description of the update environment partition for the linux system.
    pub fn find_update_part(&self) -> Option<&Partitioned> {
        let update_part_set = match self.find_update_fs() {
            Some(fs) => fs,
            None => return None,
        };

        match update_part_set.partitions.first().as_ref() {
            Some(partitions) => partitions.linux.as_ref(),
            None => None,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::variant::Variant;
    use std::{fmt::Debug, path::PathBuf};

    use super::*;

    /// Helper function to check the expected test outcome.
    fn test_expected<T>(tests: Vec<(&str, Option<T>)>)
    where
        T: Debug + PartialEq + de::DeserializeOwned,
    {
        for (json, expected) in tests {
            let result = serde_json::from_str::<T>(json);

            if expected.is_some() {
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), expected.unwrap());
            } else {
                assert!(result.is_err());
            }
        }
    }

    /// Test the deserialization of the partitioned type.
    #[test]
    fn test_load_partitioned() {
        let test_json = vec![
            (
                r#"{ "device": "mmcblk0", "partition": "p0" }"#,
                Some(Partitioned::FormatPartition {
                    device: "mmcblk0".to_string(),
                    partition: "p0".to_string(),
                }),
            ),
            (
                r#"{ "device": "mmcblk0", "offset": "0x11" }"#,
                Some(Partitioned::RawPartition {
                    device: "mmcblk0".to_string(),
                    offset: 17,
                }),
            ),
            (
                r#"{ "device": "mmcblk0", "offset": "20000" }"#,
                Some(Partitioned::RawPartition {
                    device: "mmcblk0".to_string(),
                    offset: 20000,
                }),
            ),
            (
                r#"{ "device": "0", "partition": "3" }"#,
                Some(Partitioned::FormatPartition {
                    device: "0".to_string(),
                    partition: "3".to_string(),
                }),
            ),
            (r#"{ "device": "mmcblk0" }"#, None),
            (r#"{ "partition": "p0" }"#, None),
            (r#"{ "offset": "0x11" }"#, None),
        ];

        test_expected(test_json);
    }

    /// Test the deserialization of the partition flags.
    #[test]
    fn test_load_partition_flags() {
        let test_json = vec![
            ("\"CryptoMeta\"", Some(PartitionFlags::CryptoMeta)),
            ("\"crypto_meta\"", Some(PartitionFlags::CryptoMeta)),
            ("\"CRYPTO_META\"", Some(PartitionFlags::CryptoMeta)),
            ("\"AutoDetect\"", Some(PartitionFlags::AutoDetect)),
            ("\"auto_detect\"", Some(PartitionFlags::AutoDetect)),
            ("\"AUTO_DETECT\"", Some(PartitionFlags::AutoDetect)),
            ("\"PartMeta\"", Some(PartitionFlags::PartMeta)),
            ("\"part_meta\"", Some(PartitionFlags::PartMeta)),
            ("\"PART_META\"", Some(PartitionFlags::PartMeta)),
            ("\"Overlay\"", Some(PartitionFlags::Overlay)),
            ("\"overlay\"", Some(PartitionFlags::Overlay)),
            ("\"OVERLAY\"", Some(PartitionFlags::Overlay)),
            ("\"Raw\"", Some(PartitionFlags::Raw)),
            ("\"raw\"", Some(PartitionFlags::Raw)),
            ("\"RAW\"", Some(PartitionFlags::Raw)),
        ];

        test_expected(test_json);
    }

    /// Test the loading and deserialization of a complete partition configuration.
    #[test]
    fn test_load_config() {
        let mut part_config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        part_config_path.push("../partitions.json");

        let part_config_json = std::fs::read_to_string(part_config_path).unwrap();

        let expected = PartitionConfig {
            version: "0.1.0".to_string(),
            hash_algorithm: HashAlgorithm::Sha256,
            partition_sets: vec![
                PartitionSet {
                    name: "part_conf_env".to_string(),
                    filesystem: Some("part_conf_fs".to_string()),
                    comment: "Bootloader accessible partition layout".to_string(),
                    partitions: vec![Partition {
                        linux: Some(Partitioned::RawPartition {
                            device: "mmcblk0".to_string(),
                            offset: 0x300000,
                        }),
                        bootloader: Some(Partitioned::RawPartition {
                            device: "0".to_string(),
                            offset: 0x300000,
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    name: "update_env".to_string(),
                    filesystem: Some("update_fs".to_string()),
                    comment: "Shared update environment".to_string(),
                    user_data: HashMap::from([("blob_offset".to_string(), "0x1000".to_string())]),
                    partitions: vec![Partition {
                        linux: Some(Partitioned::RawPartition {
                            device: "mmcblk0".to_string(),
                            offset: 0x200000,
                        }),
                        bootloader: Some(Partitioned::RawPartition {
                            device: "0".to_string(),
                            offset: 0x200000,
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    name: "uboot".to_string(),
                    filesystem: Some("fat32".to_string()),
                    comment: "Raspberry Pi specific bootloader partition".to_string(),
                    partitions: vec![Partition {
                        linux: Some(Partitioned::FormatPartition {
                            device: "mmcblk0".to_string(),
                            partition: "p1".to_string(),
                        }),
                        bootloader: Some(Partitioned::FormatPartition {
                            device: "0".to_string(),
                            partition: "1".to_string(),
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    id: Some(0),
                    name: "bootfs".to_string(),
                    filesystem: Some("ext2".to_string()),
                    mountpoint: Some("/boot".to_string()),
                    partitions: vec![
                        Partition {
                            variant: Some(Variant::A),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p2".to_string(),
                            }),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "2".to_string(),
                            }),
                        },
                        Partition {
                            variant: Some(Variant::B),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p3".to_string(),
                            }),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "3".to_string(),
                            }),
                        },
                    ],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    name: "home".to_string(),
                    filesystem: Some("ext2".to_string()),
                    mountpoint: Some("/home".to_string()),
                    partitions: vec![Partition {
                        linux: Some(Partitioned::FormatPartition {
                            device: "mmcblk0".to_string(),
                            partition: "p5".to_string(),
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    id: Some(1),
                    name: "rootfs".to_string(),
                    filesystem: Some("squashfs".to_string()),
                    mountpoint: Some("/".to_string()),
                    partitions: vec![
                        Partition {
                            variant: Some(Variant::A),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p6".to_string(),
                            }),
                            ..Partition::default()
                        },
                        Partition {
                            variant: Some(Variant::B),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p7".to_string(),
                            }),
                            ..Partition::default()
                        },
                    ],
                    ..PartitionSet::default()
                },
            ],
        };

        test_expected(vec![(part_config_json.as_str(), Some(expected))]);
    }
}
