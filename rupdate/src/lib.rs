// SPDX-License-Identifier: MIT

//! This is an update tool written in Rust
//!
//! The rust update implements a partition-wise A/B update using an archive of
//! partition images and a manifest file mapping these images to sets of A/B partitions
//! and providing checksums for verification. The current system state is managed in a
//! dedicated update environment shared with the bootloader.
//!
//! A/B update in a nutshell:
//! If the system is running from storage A, updates are written to B. On next boot the
//! system operates from storage B and A would be used in case an update happens.
use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use rupdate_core::{
    env::Environment,
    partitions::{PartitionConfig, Partitioned},
    state::State,
    Bundle,
};
use std::{
    env,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
};

pub const PARTITION_CONFIG_ENV: &str = "RUPDATE_PART_CONFIG";

const DEFAULT_BOOT_RETRIES: usize = 3;
const PARTITION_CONFIG_FILE: &str = "/etc/partitions.json";

#[derive(Parser, Debug)]
#[command(author = "Andreas Schickedanz <as@emlix.com>")]
#[command(version, about, long_about=None, arg_required_else_help=true)]
pub struct CliArguments {
    /// Turn on more detailed information
    #[arg(short, long)]
    pub verbose: bool,

    /// Turn on debugging information (-v is ignored if set)
    #[arg(short, long)]
    pub debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start a new update
    Update {
        /// Update bundle
        #[arg(short, long = "bundle", value_name = "BUNDLE")]
        bundle_path: Option<PathBuf>,

        /// Try to run a dry update to verify the bundle
        #[arg(short, long = "dry")]
        dry: bool,
    },
    /// Mark an installed update as ready to be tested
    Commit {
        /// Number of tries to boot the new system before automatic revert
        #[arg(short = 'r', long = "boot-retries", value_name = "NUM_RETRIES", default_value_t = DEFAULT_BOOT_RETRIES)]
        boot_retries: usize,
    },
    /// Completes an update by changing the update environment to use the new system
    Finish,
    /// Marks an update for reversion by the bootloader
    Revert,
    /// Rolls back to an old system installation
    Rollback,
    /// Print out the current update state
    State {
        /// Enable raw printing for an easier to parse output
        #[arg(short, long)]
        raw: bool,
    },
    /// Print out the complete update environment
    Env,
}

/// Executes an update
fn update<P, R>(
    bundle_path: &Option<P>,
    part_config: &PartitionConfig,
    mut env: Environment<R>,
    dry: bool,
) -> Result<()>
where
    P: AsRef<Path>,
    R: Read + Write + Seek,
{
    log::debug!("Executing an update.");
    log::info!("Reading the current update state.");

    let current_state = env.get_current_state()?;
    if current_state.state != State::Normal {
        return Err(anyhow!("Unable to update, update already in progress."));
    }

    let stream: Box<dyn BufRead> = if let Some(bundle_path) = bundle_path {
        log::debug!(
            "Reading the update bundle from {}.",
            bundle_path.as_ref().display()
        );
        Box::new(BufReader::new(File::open(bundle_path.as_ref())?))
    } else if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
        log::debug!("Reading the update bundle from stdin.");
        Box::new(BufReader::new(io::stdin()))
    } else {
        return Err(anyhow!("No valid update bundle provided."));
    };

    log::info!("Flashing the bundle.");
    let mut bundle = Bundle::new(stream)?;
    let mut new_state = bundle.flash(part_config, current_state, dry)?;

    if !dry {
        env.write_next_state(&mut new_state)
            .context("Failed to write new update state.")?;
    } else {
        log::info!("Update would have completed successfully.");
    }

    log::info!("New system installed.");

    Ok(())
}

/// Marks a previously installed update as ready to be tested
fn commit<R>(mut env: Environment<R>, boot_retries: usize) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::debug!("Committing an update to be tested.");
    log::info!("Reading the current update state.");

    let current_state = env.get_current_state()?;
    if current_state.state != State::Installed {
        return Err(anyhow!(
            "Unable to commit update, no update installed or update already committed."
        ));
    }

    let mut new_state = current_state.clone();
    new_state.state = State::Committed;
    new_state.remaining_tries = boot_retries
        .try_into()
        .context(format!("Invalid number of boot retries: {}", boot_retries))?;

    env.write_next_state(&mut new_state)
        .context("Failed to write new update state.")
}

/// Completes an update by finalizing the environment
fn finish<R>(mut env: Environment<R>) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::debug!("Completing the update.");
    log::info!("Reading the current update state.");

    let current_state = env.get_current_state()?;
    if current_state.state != State::Testing {
        return Err(anyhow!(
            "Unable to finish update, no update in progress or update is untested."
        ));
    }

    let mut new_state = current_state.clone();
    new_state.clean(true);

    env.write_next_state(&mut new_state)
        .context("Failed to write new update state.")
}

/// Marks the changes done by an uncompleted update to be reverted by the bootloader.
fn revert<R>(mut env: Environment<R>) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::debug!("Reverting the current update changes.");
    log::info!("Reading the current update state.");

    let current_state = env
        .get_current_state()
        .context("Failed to fetch currently booted state.")?;
    let mut new_state = current_state.clone();

    match current_state.state {
        State::Normal => {
            return Err(anyhow!("Unable to revert update, no update in progress."));
        }
        State::Installed | State::Committed => {
            new_state.clean(false);
        }
        State::Testing => {
            println!("Clearing boot count, please reboot to finish revert.");
            new_state.state = State::Revert;
            new_state.remaining_tries = 0;
        }
        State::Revert => {
            return Err(anyhow!(
                "Currently moving back to an older system, revert not possible."
            ));
        }
    }

    env.write_next_state(&mut new_state)
        .context("Failed to write new update state.")
}

/// Roll back to on old system version
fn rollback<R>(mut env: Environment<R>) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::info!("Rolling back to older system.");
    log::debug!("Reading the current update state.");

    let current_state = env
        .get_current_state()
        .context("Failed to fetch currently booted state.")?;

    match current_state.state {
        State::Normal => (),
        State::Revert => {
            return Err(anyhow!(
                "Already moving back to an older system, please reboot."
            ))
        }
        _ => {
            return Err(anyhow!(
                "Rollbacks are not possible during an ongoing update, use revert."
            ))
        }
    }

    let mut rollback = false;

    // Reproduce an revert state
    let mut new_state = current_state.clone();
    new_state.state = State::Revert;

    for partsel in &mut new_state.partition_selection {
        rollback |= partsel.rollback;
        partsel.affected = partsel.rollback;
        partsel.rollback = false;
    }

    if rollback {
        println!("Rollback completed, please reboot to boot into the new system.");

        env.write_next_state(&mut new_state)
            .context("Failed to write new update state.")
    } else {
        Err(anyhow!(
            "No system to roll back to or rollback not allowed."
        ))
    }
}

/// Prints the currently booted slot
fn print_state<R>(part_config: &PartitionConfig, env: Environment<R>, raw: bool) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::debug!("Printing the booted system configuration.");
    log::debug!("Fetching update states.");
    let current_state = env
        .get_current_state()
        .context("Failed to fetch currently booted state.")?;

    println!("{}", current_state.state);

    for part_set in &part_config.partition_sets {
        log::debug!("Checking selection for partition set {}.", part_set.name);
        let set_id = match part_set.id {
            Some(id) => id,
            None => continue,
        };

        let selected = part_set
            .partitions
            .iter()
            .find(|&part| {
                part.has_variant()
                    && part.variant == current_state.get_selection(&part_set.name).ok()
            })
            .with_context(|| {
                format!(
                    "Missing variant for partition set {} ({}) is not configured.",
                    part_set.name, set_id
                )
            })?;

        if let Some(linux) = &selected.linux {
            if raw {
                println!("{} {} {}", set_id, selected.variant.unwrap(), linux);
            } else {
                println!(
                    "Partition {} selected for partition set {} ({}).",
                    linux, part_set.name, set_id
                );
            }
        } else {
            return Err(anyhow!(
                "Partition variant for partition set {} ({}) is not configured.",
                part_set.name,
                set_id,
            ));
        }
    }

    Ok(())
}

/// Hex dumps the update environment
fn print_env<R>(env: Environment<R>) -> Result<()>
where
    R: Read + Write + Seek,
{
    log::debug!("Printing the update environment.");
    print!("{env}");
    Ok(())
}

/// Main application containing
pub fn app(cli_args: CliArguments) -> Result<()> {
    let part_config_path = if cfg!(debug_assertions) {
        if let Ok(path) = env::var(PARTITION_CONFIG_ENV) {
            path
        } else {
            PARTITION_CONFIG_FILE.to_owned()
        }
    } else {
        PARTITION_CONFIG_FILE.to_owned()
    };

    log::info!("Loading the partition configuration from {part_config_path}.");
    let part_config = PartitionConfig::new(&part_config_path)
        .with_context(|| format!("Failed to read partition config {}.", &part_config_path))?;
    let update_set = part_config
        .find_update_fs()
        .context("Missing update environment.")?;
    let update_part = part_config
        .find_update_part()
        .context("Missing update environment partition.")?;

    let update_device = match &update_set.mountpoint {
        Some(mountpoint) => mountpoint.to_owned(),
        None => match update_part {
            Partitioned::FormatPartition { device, partition } => {
                format!("/dev/{device}{partition}")
            }
            Partitioned::RawPartition { device, offset: _ } => format!("/dev/{}", device),
        },
    };

    log::debug!(
        "Initializing the update environment reader at {}.",
        update_device
    );

    log::info!("Opening the update environment.");
    let env_reader = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(&update_device)
        .with_context(|| {
            format!(
                "Failed to open update environment at {} for reading.",
                &update_device
            )
        })?;

    let env = Environment::from_memory(&part_config, env_reader)
        .with_context(|| format!("Failed to read update environment from {}", &update_device))?;

    match &cli_args.command {
        Some(Commands::Update { bundle_path, dry }) => update(bundle_path, &part_config, env, *dry),
        Some(Commands::Commit { boot_retries }) => commit(env, *boot_retries),
        Some(Commands::Finish) => finish(env),
        Some(Commands::Revert) => revert(env),
        Some(Commands::Rollback) => rollback(env),
        Some(Commands::State { raw }) => print_state(&part_config, env, *raw),
        Some(Commands::Env) => print_env(env),
        None => Ok(()),
    }
}
