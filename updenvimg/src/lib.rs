// SPDX-License-Identifier: MIT
use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use std::{env, fs::OpenOptions, path::PathBuf};

use rupdate_core::*;

static PARTITION_CONFIG_FILE: &str = "partitions.json";
static DEFAULT_IMAGE_PATH: &str = "update_env.img";

/// Helper function to determine the current path
fn default_path(filename: &str) -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push(filename);
    path
}

/// Clap command line arguments
#[derive(Parser, Debug)]
#[command(author = "Andreas Schickedanz <as@emlix.com>")]
#[command(version, about, long_about=None, arg_required_else_help=true)]
pub struct CliArguments {
    /// Turn on more detailed information
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    /// Turn on debugging information (-v is ignored if set)
    #[arg(short, long)]
    pub debug: bool,

    /// If set, the image includes the environment offset
    #[arg(short, long)]
    pub raw_offset: bool,

    /// Path to the partition configuration file to be used
    #[arg(short, long, value_name = "CONFIG_PATH", default_value = default_path(PARTITION_CONFIG_FILE).into_os_string())]
    pub part_config: PathBuf,

    /// Path of the generated image file
    #[arg(short, long, default_value = default_path(DEFAULT_IMAGE_PATH).into_os_string())]
    pub output: PathBuf,
}

/// Main application function
///
/// This function is seperated into its own compile unit
/// in order to allow testing the final binary.
pub fn app(cli_args: CliArguments) -> Result<()> {
    let mut part_config = PartitionConfig::new(cli_args.part_config)
        .context("Reading partition configuration failed.")?;

    if !cli_args.raw_offset {
        if let Partitioned::RawPartition { device: _, offset } = part_config
            .partition_sets
            .iter_mut()
            .find(|set| set.name == UPDATE_ENV_SET)
            .context("Failed to fetch update environment partition set.")?
            .partitions
            .first_mut()
            .context("Failed to fetch update environment file system.")?
            .linux
            .as_mut()
            .context("Failed to fetch update environment linux partition.")?
        {
            *offset = 0;
        }
    }

    let image_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(cli_args.output)
        .context("Opening update environment image failed.")?;

    let mut update_env = Environment::new(&part_config, image_file)
        .context("Parsing partition environment failed")?;
    update_env.write()
}
