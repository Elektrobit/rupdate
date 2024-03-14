// SPDX-License-Identifier: MIT
use anyhow::{Context, Result};
use clap::Parser;
use log::LevelFilter;
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

use update_tool_create_partenv::{app, CliArguments};

fn main() -> Result<()> {
    let cli_args = CliArguments::parse();

    let log_filter = if cli_args.debug {
        LevelFilter::Debug
    } else if cli_args.verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Error
    };

    let stdout = ConsoleAppender::builder()
        .target(Target::Stdout)
        .encoder(Box::new(PatternEncoder::new("{l}: {m}{n}")))
        .build();

    let log_config = log4rs::Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_filter)))
                .build("stdout", Box::new(stdout)),
        )
        .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
        .context("Configuring logging failed.")?;

    log4rs::init_config(log_config).context("Initializing logger failed: {err}.")?;

    app(cli_args).map_err(|e| {
        log::error!("{e}");
        e
    })
}
