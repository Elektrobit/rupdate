// SPDX-License-Identifier: MIT
use bincode::Options;
use rupdate_core::{env::UpdateState, state::State};
use rupdate_testing::{cmdline::exec_cmd_line, fixtures::*};
use std::{
    fs::File,
    io::{Seek, SeekFrom},
};

use update_tool_create_updenv::{app, CliArguments};

fn read_state(env_reader: File) -> UpdateState {
    bincode::options()
        .with_fixint_encoding()
        .deserialize_from::<File, UpdateState>(env_reader)
        .unwrap()
}

fn verify_default_state(update_state: &UpdateState) {
    assert!(update_state.is_valid());

    assert_eq!(update_state.magic, [b'E', b'B', b'U', b'S']);
    assert_eq!(update_state.version, 0x0000_0001);
    assert_eq!(update_state.env_revision, 0x0000_0000);
    assert_eq!(update_state.remaining_tries, -1);
    assert_eq!(update_state.state, State::Normal);
    assert_eq!(update_state.partition_selection.len(), 2);
}

#[test]
fn generate_image() {
    // Create partition config and update environment fixtures
    let part_config_file = Fixture::copy("partitions.json").unwrap();
    let env_image = Fixture::new("update_env.img");

    // Generate the update environment image
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-updenv",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--output", &env_image.path().to_string_lossy()
    ])
    .is_ok());

    // HINT: We didn't specify to seek environment offset, so we can read the first state from the start.
    let env_reader = File::open(env_image.path()).unwrap();
    let update_state1 = read_state(env_reader);
    verify_default_state(&update_state1);

    // HINT: Seek to the second state based on the value within partitions.json.
    let mut env_reader = File::open(env_image.path()).unwrap();
    env_reader.seek(SeekFrom::Start(0x1000)).unwrap();
    let update_state2 = read_state(env_reader);

    assert_eq!(update_state1, update_state2);
}

#[test]
fn use_environment_offset() {
    // Create partition config and update environment fixtures
    let part_config_file = Fixture::copy("partitions.json").unwrap();
    let env_image = Fixture::new("update_env.img");

    // Generate the update environment image
    #[rustfmt::skip]
    assert!(exec_cmd_line::<CliArguments>(app, vec![
        "update-tool-create-updenv",
        "--part-config", &part_config_file.path().to_string_lossy(),
        "--output", &env_image.path().to_string_lossy(),
        "--raw-offset"
    ])
    .is_ok());

    let mut env_reader = File::open(env_image.path()).unwrap();
    env_reader.seek(SeekFrom::Start(0x200000)).unwrap();
    let update_state1 = read_state(env_reader);
    verify_default_state(&update_state1);

    // HINT: Seek to the second state based on the value within partitions.json.
    let mut env_reader = File::open(env_image.path()).unwrap();
    env_reader.seek(SeekFrom::Start(0x201000)).unwrap();
    let update_state2 = read_state(env_reader);

    assert_eq!(update_state1, update_state2);
}
