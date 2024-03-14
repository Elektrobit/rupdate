// SPDX-License-Identifier: MIT
use anyhow::{anyhow, Context, Result};
use flate2::bufread::GzDecoder;
use ring::digest::{Context as DigestContext, Digest, SHA256};
use serde::Deserialize;
use serde_json;
use std::{
    fs::OpenOptions,
    io::{self, BufRead, Read, Seek, SeekFrom, Write},
};

use tar::Archive;

use crate::{
    env::UpdateState,
    partitions::{PartitionConfig, Partitioned},
    state::State,
};

static MANIFEST_PATH: &str = "Manifest.json";

/// Representation of a specific hash sum type.
#[derive(Deserialize, PartialEq)]
pub enum HashSum {
    #[serde(rename = "sha256")]
    Sha256(String),
}

/// Update bundle image data
///
/// The update bundle image data is a json object, which is
/// part of the update bundle manifest since version 2.
#[derive(Deserialize, PartialEq)]
pub struct Image {
    /// Name of the partition set this image is meant for (eg. rootfs, bootfs)
    name: String,
    /// Filename of the image
    filename: String,
    /// Hash sum of the image
    #[serde(flatten)]
    hash_sum: HashSum,
}

/// Update bundle manifest
///
/// The update bundle manifest is an json object containing
/// the list of included images as well as a version number
/// of the update manifest specification, which is part of the manifest
/// since version 2.
#[derive(Deserialize, PartialEq)]
pub struct Manifest {
    /// Version of the installed system
    version: String,
    /// Whether or not a rollback is allowed for this update (no for security updates)
    #[serde(rename = "rollback-allowed")]
    rollback_allowed: bool,
    /// List of images included with this update
    images: Vec<Image>,
}

impl Manifest {
    /// Create a new manifest
    ///
    /// Setups a new manifest by parsing the json object
    /// returned by the provided reader.
    pub fn new(reader: impl Read) -> Result<Self> {
        Ok(serde_json::from_reader(reader)?)
    }

    /// Returns the checksum for the given image
    ///
    /// Returns the checksum for the specified image or None,
    /// if a checksum for this image does not exist.
    pub fn get_checksum(&self, name: &str) -> Option<&String> {
        if let Some(image) = self.images.iter().find(|&image| image.name == name) {
            match &image.hash_sum {
                HashSum::Sha256(sha256) => Some(sha256),
            }
        } else {
            None
        }
    }

    /// Find an image name by the name of the corresponding partition set
    ///
    /// Returns the name of the image
    ///
    /// # Error
    ///
    /// Returns an error variant if no matching partition set could be found.
    pub fn find_image(&self, part_set_name: &str) -> Result<&Image> {
        self.images
            .iter()
            .find(|&image| image.name == part_set_name)
            .ok_or_else(|| anyhow!("Failed to find image for partition set {part_set_name}."))
    }
}

/// The update bundle
///
/// The update bundle is a tar archive, which may be compressed using the
/// gzip compression algorithm. This archive contains a json encoded manifest,
/// specifying the images included with the update and the corresponding checksums.
pub struct Bundle(Archive<Box<dyn BufRead>>);

impl Bundle {
    /// Create a new Bundle instance.
    ///
    /// Returns a new Bundle instance, by wrapping the provided buffer.
    ///
    /// # Error
    ///
    /// Returns an error variant if the parsing of the provided
    /// input fails.
    pub fn new(mut stream: Box<dyn BufRead>) -> Result<Self> {
        let tar: Box<dyn BufRead> = if Self::is_gzipped(stream.as_mut())? {
            Box::new(io::BufReader::new(GzDecoder::new(stream)))
        } else {
            stream
        };

        Ok(Self(Archive::new(tar)))
    }

    /// Writes the images from the update bundle into the corresponding partition sets.
    ///
    /// Extracts the manifest from a given bundle and iterates over all
    /// specified partition set entries. If a correspoding partition set is found
    /// within the partition config, the image related to this set gets flashed
    /// to the currently inactive partition. Finally a new update state is generated and
    /// returned.
    ///
    /// # Error
    ///
    /// Returns an error variant if flashing fails.
    pub fn flash(
        &mut self,
        part_config: &PartitionConfig,
        current_state: &UpdateState,
        dry: bool,
    ) -> Result<UpdateState> {
        if dry {
            log::info!("Executing a dry update - Nothing will change.")
        }

        log::info!("Reading the update manifest.");
        let (manifest, entries) = self.context()?;

        let mut new_state = current_state.clone();
        new_state.disable_rollback();

        for (partition_set, entry) in entries.enumerate() {
            match entry {
                Ok(mut entry) => {
                    let part_set = part_config
                        .partition_sets
                        .iter()
                        .find(|&set| set.id.is_some() && set.id.unwrap() == partition_set as u32)
                        .with_context(|| {
                            format!("Failed to find partition set {partition_set}.")
                        })?;

                    log::debug!("Checking for image for partition set {}.", part_set.name);
                    let image = &manifest.find_image(&part_set.name)?.filename;

                    log::debug!(
                        "Checking for partition for partition set {}.",
                        part_set.name
                    );

                    let partition = part_set
                        .partitions
                        .iter()
                        .find(|&part| {
                            part.has_variant()
                                && *part.variant.as_ref().unwrap()
                                    != current_state.get_selection(&part_set.name).unwrap()
                        })
                        .with_context(|| {
                            format!("Failed to detect partition to flash {image} to.")
                        })?;

                    let linux_part = partition
                        .linux
                        .as_ref()
                        .with_context(|| format!("Failed to find linux partition for {image}."))?;

                    log::debug!("Extracting {image} to {linux_part}.");

                    let digest = Bundle::extract(&mut entry, linux_part, dry)?;
                    let expected = ring::test::from_hex(
                        manifest
                            .get_checksum(part_set.name.as_str())
                            .with_context(|| format!("Missing hash sum for {image}."))?,
                    )
                    .map_err(|_| anyhow!("Failed to calculate hash sum for {image}."))?;

                    log::debug!("Checking checksum of {}.", image);
                    if digest.as_ref() != expected {
                        return Err(anyhow!("Invalid hash sum given for {image}."));
                    }

                    if manifest.rollback_allowed {
                        new_state.allow_rollback(&part_set.name)?;
                    }

                    log::debug!("Updating partition layout.");
                    new_state.mark_new(&part_set.name)?;

                    if dry {
                        log::debug!("Would have written {image} to {linux_part}.");
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }

        new_state.state = State::Installed;
        new_state
            .update_hash_sum()
            .context("Failed to update hash sum of update state")?;

        if *current_state == new_state {
            return Err(anyhow!(
                "No partitions have been updated: Missing partitions or hash sums."
            ));
        }

        Ok(new_state)
    }

    /// Extract the current entry.
    ///
    /// Extracts the current archive entry to the specified partition and
    /// verifies the checksum of the written image.
    ///
    /// # Error
    ///
    /// Returns an error variant if reading the image, writing the image or the
    /// image verification using the checksum fails.
    fn extract(
        entry: &mut tar::Entry<Box<dyn BufRead>>,
        partition: &Partitioned,
        dry: bool,
    ) -> Result<Digest> {
        let (partition, partition_offset) = match partition {
            Partitioned::FormatPartition { device, partition } => {
                (format!("/dev/{}{}", device, partition), 0x00)
            }
            Partitioned::RawPartition { device, offset } => (format!("/dev/{}", device), *offset),
        };

        let mut device = OpenOptions::new()
            .write(true)
            .open(&partition)
            .with_context(|| format!("Failed to open {partition} for flashing."))?;
        device.seek(SeekFrom::Start(partition_offset))?;

        let mut hash_ctx = DigestContext::new(&SHA256);
        let mut buf: [u8; 0x2000] = [0x00; 0x2000];
        let mut file_size = entry.size();

        while file_size > 0 {
            let bytes_read = entry.read(&mut buf[..])?;

            hash_ctx.update(&buf[..bytes_read]);

            if !dry {
                device.write_all(&buf[..bytes_read])?;
            }

            file_size -= bytes_read as u64;
        }

        Ok(hash_ctx.finish())
    }

    /// Return the context of the bundle.
    ///
    /// Returns the update bundle manifest, which describes the contents
    /// of the update, and the image entries.
    ///
    /// # Error
    ///
    /// Returns an error variant if the bundle is not accessible or
    /// there is no or an invalid manifest.
    fn context(&mut self) -> Result<(Manifest, tar::Entries<Box<dyn BufRead>>)> {
        let mut entries = self.0.entries()?;
        let manifest_entry = entries
            .next()
            .context("Update bundle manifest missing.")?
            .context("Accessing the update bundle failed.")?;
        let manifest = if manifest_entry
            .path()
            .context("First file in bundle is not the manifest.")?
            .ends_with(MANIFEST_PATH)
        {
            Manifest::new(manifest_entry)?
        } else {
            return Err(anyhow!("First file in bundle is not the manifest."));
        };

        Ok((manifest, entries))
    }

    /// Checks if the bundle is compressed.
    ///
    /// Returns true if the first two bytes of the given stream
    /// match the two bytes 0x1F and 0x8B, which is the header
    /// of a gzip compressed file.
    ///
    /// # Error
    ///
    /// Returns an error variant if reading fails.
    fn is_gzipped<R>(reader: &mut R) -> Result<bool>
    where
        R: ?Sized + BufRead,
    {
        // fill_buf does not consume the read bytes, which is perfect for this test
        Ok(reader.fill_buf()?.starts_with(&[0x1f, 0x8b]))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json;

    /// Test deserialization of an image description.
    #[test]
    fn test_deserialize_image() {
        let manifest_json = r##"
            {
                "name": "rootfs",
                "filename": "rootfs.img",
                "sha256": "31533a2aad5ebdf2c34fe03746fa2782693415357ee50fc50aab4e58ca6792ce"
            }
"##;
        let manifest: Image = serde_json::from_str(manifest_json).unwrap();
        assert_eq!(manifest.name, "rootfs");
        assert_eq!(manifest.filename, "rootfs.img");
        assert!(matches!(manifest.hash_sum, HashSum::Sha256(_)));
    }

    /// Test deserialization of an update manifest.
    #[test]
    fn test_deserialize_manifest() {
        let manifest_json = r##"
        {
            "version": "2.0",
            "rollback-allowed": false,
            "images": [
                {
                    "name": "rootfs",
                    "filename": "rootfs.img",
                    "sha256": "31533a2aad5ebdf2c34fe03746fa2782693415357ee50fc50aab4e58ca6792ce"
                },
                {
                    "name": "bootfs",
                    "filename": "bootfs.img",
                    "sha256": "31533a2aad5ebdf2c34fe03746fa2782693415357ee50fc50aab4e58ca6792ce"
                }
            ]
        }
"##;
        let manifest: Manifest = serde_json::from_str(manifest_json).unwrap();
        assert_eq!(manifest.version, "2.0");
    }

    /// Test deserialization of the image checksum.
    #[test]
    fn test_deserialize_checksum() {
        let man = r##"{ "version": "2.0", "rollback-allowed": true, "images": [ { "name": "bootfs", "filename": "bootfs.img", "sha256": "d3adc0ff" } ] }"##;
        let manifest: Manifest = serde_json::from_str(man).unwrap();
        assert_eq!(manifest.get_checksum("rootfs"), None);

        let man_sha256 = r##"{ "version": "2.0", "rollback-allowed": false, "images": [ { "name": "bootfs", "filename": "bootfs.img", "sha256": "c0ffd00d" } ] }"##;
        let manifest: Manifest = serde_json::from_str(man_sha256).unwrap();
        assert_eq!(manifest.get_checksum("bootfs").unwrap(), "c0ffd00d");
    }
}
