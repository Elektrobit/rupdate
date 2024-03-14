// SPDX-License-Identifier: MIT
use crate::{
    fixed_string::FixedString,
    hash_sum::HashSum,
    hex_dump::HexDump,
    partitions::{PartitionConfig, Partitioned},
    variant::Variant,
};
use anyhow::{anyhow, Context, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{Read, Seek, SeekFrom, Write},
    ops::Deref,
};

pub const PART_CONF_ENV_FILESYSTEM: &str = "part_conf_fs";
pub const PART_CONF_ENV_SET: &str = "part_conf_env";
pub const PART_CONF_MAGIC: &[u8; 4] = &[b'E', b'B', b'P', b'C'];

/// Partition set defined by a name and a unique id.
#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug, PartialEq))]
pub struct SetDescriptor {
    /// Numeric id
    pub id: u8,
    /// Partition set name (36 byte ascii string)
    pub name: FixedString<36>,
}

/// Description of a partition from a linux and bootloader perspective.
#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug, PartialEq))]
pub struct PartitionDescriptor {
    /// Variant (either A = 0x00 or B = 0x01)
    pub variant: Variant,
    /// Numeric partition set id
    pub set_id: u8,
    /// Bootloader device id (36 byte ascii string - also fits UUIDs)
    pub bootloader_device_id: FixedString<36>,
    /// Bootloader partition id (36 byte ascii string - also fits UUIDs)
    pub bootloader_partition_id: FixedString<36>,
    /// Linux device id (36 byte ascii string - also fits UUIDs)
    pub linux_device_id: FixedString<36>,
    /// Linux partition id (36 byte ascii string - also fits UUIDs)
    pub linux_partition_id: FixedString<36>,
}

/// Transparent data type to capsulate the partition environment data.
///
/// The encapsulation of the partition environment data into a
/// separate type eases the serialization of the data independent
/// of the corresponding hash sum stored along this data.
#[derive(Deserialize, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug, PartialEq))]
pub struct PartitionEnvironmentData {
    /// 4 Byte magic number
    pub magic: [u8; 4],
    /// 4 byte version
    pub version: u32,
    /// List of set descriptors
    pub sets: Vec<SetDescriptor>,
    /// List of partitions
    pub partitions: Vec<PartitionDescriptor>,
}

impl Default for PartitionEnvironmentData {
    fn default() -> PartitionEnvironmentData {
        Self {
            magic: PART_CONF_MAGIC.to_owned(),
            version: 0x00000001,
            sets: Vec::new(),
            partitions: Vec::new(),
        }
    }
}

/// Partition environment combining the environment data and the corresponding hash sum.
///
/// The partition environment is the bootloader accessible equivalent to the partition
/// configuration, which is placed within the rootfs to provide partition information to
/// the update tool.
#[derive(Default, Deserialize, Serialize)]
pub struct PartitionEnvironment {
    /// The actual data
    pub data: PartitionEnvironmentData,
    /// Checksum
    pub checksum: HashSum,
}

/// Allow transparent access to the internal data of an partition environment
impl Deref for PartitionEnvironment {
    type Target = PartitionEnvironmentData;
    #[inline]
    fn deref(&self) -> &PartitionEnvironmentData {
        &self.data
    }
}

impl HexDump for PartitionEnvironment {}

/// Implement display trait for the partition environment as hex dump.
impl fmt::Display for PartitionEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.hex_dump(f)
            .context("Failed to serialize partition environment.")
            .map_err(|_| fmt::Error)
    }
}

impl PartitionEnvironment {
    /// Generates a partition environment for the given partition sets based on a partition config.
    ///
    /// Parses the given partition config and extracts the relevant data on the given
    /// partition sets to be stored within the partition environment.
    ///
    /// # Error
    ///
    /// Returns an error variant if generating the partition environment fails.
    pub fn from_config(part_config: &PartitionConfig, set_names: Vec<String>) -> Result<Self> {
        let mut part_env = PartitionEnvironment::default();
        let part_env_data = &mut part_env.data;

        for set_name in set_names.iter() {
            let set = part_config.find_set(set_name)
                .with_context(|| format!("Failed to find partition set '{}' in partition config", &set_name))?;
            part_env_data.sets.push(SetDescriptor {
                id: set
                    .id
                    .with_context(|| format!("Failed to find ID for partition set '{}'.", &set_name))?
                    .try_into()
                    .with_context(|| format!("Failed to convert ID of partition set '{}'", &set_name))?,
                name: set.name.parse()?,
            });
            for part in set.partitions.iter() {
                part_env_data
                    .partitions
                    .push(match (&part.bootloader, &part.linux) {
                        (
                            Some(Partitioned::FormatPartition {
                                device: bootloader_device,
                                partition: bootloader_partition,
                            }),
                            Some(Partitioned::FormatPartition {
                                device: linux_device_id,
                                partition: linux_partition_id,
                            }),
                        ) => PartitionDescriptor {
                            set_id: set.id.with_context(|| {
                                format!("Missing partition set id for '{}'.", &set_name)
                            })? as u8,
                            variant: part.variant.unwrap_or_default(),
                            bootloader_device_id: bootloader_device.parse()?,
                            bootloader_partition_id: bootloader_partition.parse()?,
                            linux_device_id: linux_device_id.parse()?,
                            linux_partition_id: linux_partition_id.parse()?,
                        },
                        _ => return Err(anyhow!(
                            "Failed to find bootloader/linux partitions for partition set '{set_name}'."
                        )),
                    });
            }
        }

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&part_env_data)?;
        part_env.checksum =
            HashSum::generate(serialized.as_slice(), part_config.hash_algorithm.clone())?;

        Ok(part_env)
    }

    /// Returns a new instance of the Partition Configuration Environment.
    ///
    /// Initializes the environment based on the given partition configuration
    /// and device handler and reads the environment, placed in raw memory in front of the
    /// bootloader.
    ///
    /// # Error
    ///
    /// Returns an error variant if reading of partition configuration environment failed.
    pub fn from_memory<T>(dp: T) -> Result<Self>
    where
        T: Read + Write + Seek,
    {
        Ok(bincode::options()
            .with_fixint_encoding()
            .deserialize_from::<T, PartitionEnvironment>(dp)?)
    }

    /// Seeks to the offset within the partition the partition environment should be placed into.
    ///
    /// Reads the information needed to write the partition environment from the
    /// given partition configuration and seeks to the specified offset within the target partition.
    ///
    /// # Error
    ///
    /// Returns an error variant, if seeking fails.
    fn seek<T>(part_config: &PartitionConfig, dp: &mut T) -> Result<()>
    where
        T: Read + Write + Seek,
    {
        let config_part_set = part_config
            .find_set(PART_CONF_ENV_FILESYSTEM)
            .context("Failed to find definition of parition config filesystem set in partition config.")?;

        if config_part_set.filesystem.is_none()
            || config_part_set.filesystem != Some(PART_CONF_ENV_FILESYSTEM.to_string())
        {
            return Err(anyhow!(
                "Failed to check file system type of partition config set."
            ));
        }

        let config_part = match config_part_set.partitions.first() {
            Some(partitions) => partitions.bootloader.as_ref()
                .context("Failed to find bootloader parition of parition config filesystem.")?,
            None => return Err(anyhow!("No partitions specified for partition config set.")),
        };

        if let Partitioned::RawPartition { device: _, offset } = config_part {
            dp.seek(SeekFrom::Start(*offset))?;
        } else {
            return Err(anyhow!("Partition type not seekable."));
        }

        Ok(())
    }

    /// Seeks to the right offset within the given output stream and writes the partition environment.
    ///
    /// Depending on the way the system image is created, it might be useful to write the
    /// partition environment directly to the correct offset. Thus write() seeks to the correct
    /// offset and writes the partition environment to the given output stream.
    ///
    /// # Error
    ///
    /// Returns an error variant, if writing the partition environment fails.
    pub fn write<T>(&self, part_config: &PartitionConfig, dp: &mut T) -> Result<()>
    where
        T: Read + Write + Seek,
    {
        Self::seek(part_config, dp)?;

        self.write_image(dp)
    }

    /// Writes an partition environment image to the given output stream.
    ///
    /// Writes the partition environment to the given output stream without
    /// seeking to the offset specified in the partition config. This is useful
    /// to write images, which will be written to the correct offset during the
    /// image assembly process.
    ///
    /// # Error
    ///
    /// Returns an error variant, if writing the image fails.
    pub fn write_image<T>(&self, dp: &mut T) -> Result<()>
    where
        T: Read + Write + Seek,
    {
        let raw = self.raw()?;
        dp.write_all(raw.as_slice())?;

        Ok(())
    }

    /// Returns a binary encoded representation of the partition environment.
    ///
    /// The returned binary encoded representation of the partition environment
    /// uses the bincode encoding standard to encode the data.
    ///
    /// # Error
    ///
    /// Returns an error, if binary encoding the partition environment fails.
    fn raw(&self) -> Result<Vec<u8>> {
        Ok(bincode::options().with_fixint_encoding().serialize(&self)?)
    }
}

#[cfg(test)]
mod test {
    use super::{PartitionEnvironment, SetDescriptor, PART_CONF_ENV_FILESYSTEM, PART_CONF_ENV_SET};

    use crate::{
        part_env::{FixedString, PartitionDescriptor, PartitionEnvironmentData, PART_CONF_MAGIC},
        partitions::{Partition, PartitionConfig, PartitionSet, Partitioned},
        variant::Variant,
    };
    use bincode::Options;

    /// Provides a default partition configuration for testing.
    fn default_part_config() -> PartitionConfig {
        PartitionConfig {
            partition_sets: vec![
                PartitionSet {
                    name: PART_CONF_ENV_SET.to_string(),
                    filesystem: Some(PART_CONF_ENV_FILESYSTEM.to_string()),
                    partitions: vec![Partition {
                        bootloader: Some(Partitioned::RawPartition {
                            device: "mmcblk0".to_string(),
                            offset: 0xdeadb33f,
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    id: Some(0),
                    name: "bootfs".to_string(),
                    partitions: vec![
                        Partition {
                            variant: Some(Variant::A),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "0".to_string(),
                            }),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p0".to_string(),
                            }),
                        },
                        Partition {
                            variant: Some(Variant::B),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "1".to_string(),
                            }),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p1".to_string(),
                            }),
                        },
                    ],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    id: Some(1),
                    name: "rootfs".to_string(),
                    filesystem: Some("ext4".to_string()),
                    partitions: vec![
                        Partition {
                            variant: Some(Variant::A),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "2".to_string(),
                            }),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p2".to_string(),
                            }),
                        },
                        Partition {
                            variant: Some(Variant::B),
                            bootloader: Some(Partitioned::FormatPartition {
                                device: "0".to_string(),
                                partition: "4".to_string(),
                            }),
                            linux: Some(Partitioned::FormatPartition {
                                device: "mmcblk0".to_string(),
                                partition: "p4".to_string(),
                            }),
                        },
                    ],
                    ..PartitionSet::default()
                },
                PartitionSet {
                    name: "appfs".to_string(),
                    filesystem: Some("zfs".to_string()),
                    partitions: vec![Partition {
                        bootloader: Some(Partitioned::FormatPartition {
                            device: "0".to_string(),
                            partition: "5".to_string(),
                        }),
                        ..Partition::default()
                    }],
                    ..PartitionSet::default()
                },
            ],
            ..PartitionConfig::default()
        }
    }

    /// Test serialization of partition set descriptors.
    #[test]
    fn test_serialize_set_descriptor() {
        let set = SetDescriptor {
            id: 7,
            name: "bootfs".parse().unwrap(),
        };

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&set)
            .unwrap();

        let mut expected = [0u8; std::mem::size_of::<FixedString<36>>() + 1];
        expected[..7].copy_from_slice(&[7, b'b', b'o', b'o', b't', b'f', b's']);

        assert_eq!(serialized.as_slice(), &expected);
    }

    /// Test serialization of partition descriptors.
    #[test]
    fn test_serialize_partition_descriptor() {
        let partition = PartitionDescriptor {
            variant: Variant::B,                           // 1 byte
            set_id: 2,                                     // 1 byte
            bootloader_device_id: "3".parse().unwrap(),    // 36 bytes
            bootloader_partition_id: "7".parse().unwrap(), // 36 bytes
            linux_device_id: "mmcblk3".parse().unwrap(),   // 36 bytes
            linux_partition_id: "p7".parse().unwrap(),     // 36 bytes
        };

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&partition)
            .unwrap();

        let mut expected = [0u8; 146];
        expected[..3].copy_from_slice(&[1, 2, b'3']);
        expected[38] = b'7';
        expected[74..81].copy_from_slice(&[b'm', b'm', b'c', b'b', b'l', b'k', b'3']);
        expected[110..112].copy_from_slice(&[b'p', b'7']);

        assert_eq!(serialized.as_slice(), &expected);
    }

    /// Test serialization of partition environment data.
    #[test]
    fn test_serialize_partition_environment_data() {
        let data = PartitionEnvironmentData {
            sets: vec![
                // additional 8 bytes for vec size
                SetDescriptor {
                    id: 1,
                    name: "bootfs".parse().unwrap(),
                },
                SetDescriptor {
                    id: 2,
                    name: "rootfs".parse().unwrap(),
                },
            ],
            partitions: vec![
                // additional 8 bytes for vec size
                PartitionDescriptor {
                    variant: Variant::A,                           // 4 byte
                    set_id: 1,                                     // 1 byte
                    bootloader_device_id: "0".parse().unwrap(),    // 32 bytes
                    bootloader_partition_id: "0".parse().unwrap(), // 32 bytes
                    linux_device_id: "mmcblk0".parse().unwrap(),   // 32 bytes
                    linux_partition_id: "p0".parse().unwrap(),     // 32 bytes
                },
                PartitionDescriptor {
                    variant: Variant::B,                           // 4 byte
                    set_id: 1,                                     // 1 byte
                    bootloader_device_id: "0".parse().unwrap(),    // 32 bytes
                    bootloader_partition_id: "1".parse().unwrap(), // 32 bytes
                    linux_device_id: "mmcblk0".parse().unwrap(),   // 32 bytes
                    linux_partition_id: "p1".parse().unwrap(),     // 32 bytes
                },
                PartitionDescriptor {
                    variant: Variant::A,                           // 4 byte
                    set_id: 2,                                     // 1 byte
                    bootloader_device_id: "0".parse().unwrap(),    // 32 bytes
                    bootloader_partition_id: "2".parse().unwrap(), // 32 bytes
                    linux_device_id: "mmcblk0".parse().unwrap(),   // 32 bytes
                    linux_partition_id: "p2".parse().unwrap(),     // 32 bytes
                },
                PartitionDescriptor {
                    variant: Variant::B,                           // 4 byte
                    set_id: 2,                                     // 1 byte
                    bootloader_device_id: "0".parse().unwrap(),    // 32 bytes
                    bootloader_partition_id: "4".parse().unwrap(), // 32 bytes
                    linux_device_id: "mmcblk0".parse().unwrap(),   // 32 bytes
                    linux_partition_id: "p4".parse().unwrap(),     // 32 bytes
                },
            ],
            ..PartitionEnvironmentData::default()
        };

        let serialized = bincode::options()
            .with_fixint_encoding()
            .serialize(&data)
            .unwrap();
        let deserialized: PartitionEnvironmentData = bincode::options()
            .with_fixint_encoding()
            .deserialize(&serialized)
            .unwrap();

        assert_eq!(data, deserialized);
    }

    /// Test generation of a partition environment based on a partition configuration.
    #[test]
    fn test_from_config_success() {
        let part_config = default_part_config();
        let part_env = PartitionEnvironment::from_config(
            &part_config,
            vec!["bootfs".to_string(), "rootfs".to_string()],
        );

        assert!(part_env.is_ok());

        if let Ok(part_env) = part_env {
            assert_eq!(part_env.data.magic, *PART_CONF_MAGIC);
            assert_eq!(part_env.data.version, 0x00000001);
            assert_eq!(part_env.data.sets.len(), 2);
            assert_eq!(part_env.data.partitions.len(), 4);
        }
    }
}
