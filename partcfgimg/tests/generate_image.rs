// SPDX-License-Identifier: MIT
use bincode::Options;
use rupdate_core::PartitionEnvironment;
use rupdate_testing::{cmdline::exec_cmd_line, fixtures::*};
use std::fs::File;

use update_tool_create_partenv::{app, CliArguments};

/// Read the generated partition environment from a fixture
fn read_part_env(part_env_image: &Fixture) -> PartitionEnvironment {
    let env_reader = File::open(part_env_image.path()).unwrap();
    bincode::options()
        .with_fixint_encoding()
        .deserialize_from::<File, PartitionEnvironment>(env_reader)
        .unwrap()
}

/// Test the image generation
#[test]
fn generate_image() {
    // Create partition config and partition environment fixtures
    let part_config_file = Fixture::copy("partitions.json").unwrap();
    let part_env_image = Fixture::new("partition_env.img");

    // Generate the partition environment image
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-partenv", "image",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--sets=bootfs,rootfs",
        "--output", &part_env_image.path().to_string_lossy()
    ])
    .is_ok());

    let part_env = read_part_env(&part_env_image);

    assert_eq!(part_env.magic, [b'E', b'B', b'P', b'C']);
    assert_eq!(part_env.version, 0x0000_0001);
    assert_eq!(part_env.sets.len(), 2);
    assert_eq!(part_env.partitions.len(), 4);
}

/// Test the different options to list partition sets
#[test]
fn listing_sets() {
    // Create partition config and partition environment fixtures
    let part_config_file = Fixture::copy("partitions.json").unwrap();
    let part_env_image_list = Fixture::new("partition_env_list.img");
    let part_env_image_single = Fixture::new("partition_env_single.img");

    // Listed sets
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-partenv", "image",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--sets=bootfs,rootfs",
        "--output", &part_env_image_list.path().to_string_lossy()
    ])
    .is_ok());

    // Single sets
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-partenv", "image",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--sets", "bootfs", "--sets", "rootfs",
        "--output", &part_env_image_single.path().to_string_lossy()
    ])
    .is_ok());
}

/// Test overwriting an existing image file
#[test]
fn overwrite_existing_image() {
    // Create partition config and partition environment fixtures
    let part_config_file = Fixture::copy("partitions.json").unwrap();
    let part_env_image = Fixture::new("partition_env.img");

    // Create the image once
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-partenv", "image",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--sets=bootfs,rootfs",
        "--output", &part_env_image.path().to_string_lossy()
    ])
    .is_ok());

    // Overwrite the existing image
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-partenv", "image",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--sets=bootfs,rootfs",
        "--output", &part_env_image.path().to_string_lossy()
    ])
    .is_ok());
}
