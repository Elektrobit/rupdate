// SPDX-License-Identifier: MIT

//! This generates an partition environment image given a partition configuration
//! of which former contains all information about the system's partition layout
//! needed by a bootloader or hypervisor to boot the system.
//!
//! To keep the generated as small as possible, the partition sets defined by
//! the partition configuration, that shall be included in the environment
//! must be specified when calling the generator.
//!
//! For more details on the differences on the partition configuration JSON format
//! and the bincode encoded partition environment please refer to the project'S README.
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rupdate_core::*;
use std::{fs::OpenOptions, path::Path};

/// Default filename of the partition configuration
const DEFAULT_PARTITION_CONFIG: &str = "partitions.json";
/// Default filename of the partition environment image
const DEFAULT_ENVIRONMENT_IMAGE: &str = "partition_config.img";

/// Command line arguments
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
    command: Commands,
}

/// Application commands
#[derive(Debug, Subcommand)]
enum Commands {
    /// Print out the partition configuration environment that would be generated
    Print {
        /// Path to the partition configuration file to be used
        #[arg(short, long, value_name = "CONFIG_PATH")]
        part_config: Option<String>,
        /// Names of sets to be included in the partition configuration
        #[arg(short, long)]
        sets: Vec<String>,
    },
    /// Create an image based on the given partition config
    Image {
        /// Path to the partition configuration file to be used
        #[arg(short, long, value_name = "CONFIG_PATH")]
        part_config: Option<String>,
        /// Names of sets to be included in the partition configuration
        #[arg(short, long, use_value_delimiter = true, value_delimiter = ',')]
        sets: Vec<String>,
        /// Path of the generated image file
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Prints out a hex representation of the partition environment that would be generated.
///
/// Based on the given partition configuration and the selected sets
/// a partition environment is generated which is then dumped in a
/// hexadecimal representation for analysis. This does not save the generated
/// environment to a file.
fn print(sets: &[String], part_config: &Option<String>) -> Result<()> {
    let config_path = match part_config {
        Some(path) => path.as_str(),
        None => DEFAULT_PARTITION_CONFIG,
    };

    log::info!("Loading the partition configuration from {config_path}.");

    let part_config = PartitionConfig::new(Path::new(config_path))
        .context("Reading partition configuration failed.")?;

    let part_env = PartitionEnvironment::from_config(&part_config, sets.into())
        .context("Parsing partition environment failed")?;

    println!("{}", part_env);

    Ok(())
}

/// Generates a partition environment image.
///
/// Based on the given partition configuration and the selected sets
/// a partition environment is generated and written to the specified
/// output file.
fn image(sets: &[String], part_config: &Option<String>, output: &Option<String>) -> Result<()> {
    let config_path = match part_config {
        Some(path) => path.as_str(),
        None => DEFAULT_PARTITION_CONFIG,
    };
    let image_path = match output {
        Some(path) => path.as_str(),
        None => DEFAULT_ENVIRONMENT_IMAGE,
    };

    log::info!("Loading the partition configuration from {config_path}.");

    let part_config = PartitionConfig::new(Path::new(config_path))
        .context("Reading partition configuration failed.")?;

    let part_env = PartitionEnvironment::from_config(&part_config, sets.into())
        .context("Generating partition environment failed.")?;

    let mut image_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(image_path)
        .context("Opening partition environment image failed.")?;
    part_env
        .write_image(&mut image_file)
        .with_context(|| format!("Failed to write partition environment to {}.", config_path))
}

/// Main application containing
pub fn app(cli_args: CliArguments) -> Result<()> {
    match &cli_args.command {
        Commands::Print { sets, part_config } => print(sets, part_config),
        Commands::Image {
            sets,
            part_config,
            output,
        } => image(sets, part_config, output),
    }
}
