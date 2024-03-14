// SPDX-License-Identifier: MIT
use rupdate_core::{state::State, Environment, PartitionConfig, UPDATE_ENV_SET};
use rupdate_testing::{cmdline::exec_cmd_line, fixtures::*};
use std::{
    env,
    fs::{File, OpenOptions},
    io::Write,
};

use rupdate::{app, CliArguments, PARTITION_CONFIG_ENV};

struct TestContext {
    part_config: Fixture,
    update_env: Fixture,
    update_bundle: Fixture,
}

impl Default for TestContext {
    fn default() -> Self {
        Self {
            part_config: Fixture::copy("partitions.json").unwrap(),
            update_env: Fixture::new("update_env.img"),
            update_bundle: Fixture::copy("update_bundle.tar.gz").unwrap(),
        }
    }
}

/// Setup an update environment
fn update_env_init(state: State, part_config: &PartitionConfig, update_env: &Fixture) {
    // Write the update environment to the provided fixture
    let update_env_img = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(update_env.path())
        .unwrap();

    let mut update_env = Environment::new(part_config, update_env_img).unwrap();
    update_env.write().unwrap();

    if state != State::Normal {
        let mut new_state = update_env.get_current_state().unwrap().clone();
        new_state.state = state;
        update_env.write_next_state(&mut new_state).unwrap();
    }
}

fn inject_update_env(
    part_config: &mut PartitionConfig,
    part_config_file: &Fixture,
    update_env: &Fixture,
) {
    // Set the mountpoint of the update environment in the partition config
    // to our fake update environment image.
    let mut update_fs = part_config
        .partition_sets
        .iter_mut()
        .find(|set| set.name == UPDATE_ENV_SET)
        .unwrap();
    update_fs.mountpoint = Some(update_env.path().display().to_string());

    let part_conf_json = serde_json::to_string(&part_config).unwrap();
    let mut part_conf_writer = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(part_config_file.path())
        .unwrap();
    part_conf_writer
        .write_all(part_conf_json.as_bytes())
        .unwrap();

    // Inject the changed parition config.
    env::set_var(PARTITION_CONFIG_ENV, part_config_file.path());
}

/// Read the current update environment from a fixture
fn read_update_env<'a>(
    part_config: &'a PartitionConfig,
    update_env: &'a Fixture,
) -> Environment<'a, File> {
    let env_reader = OpenOptions::new()
        .read(true)
        .truncate(false)
        .open(update_env.path())
        .unwrap();

    Environment::from_memory(part_config, env_reader).unwrap()
}

/// Common test Setup
fn setup(state: State) -> TestContext {
    let ctx = TestContext::default();

    // Create partition config, update bundle, update environment and partition fixtures
    let mut part_config = PartitionConfig::new(ctx.part_config.path()).unwrap();

    // Inject the path into the partition config
    inject_update_env(&mut part_config, &ctx.part_config, &ctx.update_env);

    // Initialize a default update environment
    update_env_init(state, &part_config, &ctx.update_env);

    ctx
}

/// Test the image flashing
fn test_state_change(initial_state: State, final_state: State, cmd_line: &[&str]) {
    let ctx = setup(initial_state);

    let part_config = PartitionConfig::new(ctx.part_config.path()).unwrap();
    let update_env = read_update_env(&part_config, &ctx.update_env);

    assert_eq!(update_env.get_current_state().unwrap().state, initial_state);

    let mut cmd_line: Vec<String> = cmd_line.iter().map(|&s| s.into()).collect();
    if cmd_line[1] == "update" {
        cmd_line.push(ctx.update_bundle.path().to_string_lossy().to_string());
    }

    // Install a new system
    assert!(
        exec_cmd_line::<CliArguments>(app, cmd_line.iter().map(|s| s.as_str()).collect()).is_ok()
    );

    // Read changed environment
    let update_env = read_update_env(&part_config, &ctx.update_env);

    assert_eq!(update_env.get_current_state().unwrap().state, final_state);
}

#[test]
fn test_state_changes() {
    test_state_change(
        State::Normal,
        State::Installed,
        &["rupdate", "update", "--bundle"],
    );

    // Test committing an update
    test_state_change(State::Installed, State::Committed, &["rupdate", "commit"]);

    // Test finishing an update
    test_state_change(State::Testing, State::Normal, &["rupdate", "finish"]);
}
