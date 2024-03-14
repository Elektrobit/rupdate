// SPDX-License-Identifier: MIT
use clap::Parser;
use log::LevelFilter;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

use rupdate::{app, CliArguments};

fn main() {
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
    let log_file = match FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {m}{n}",
        )))
        .build("/var/log/rupdate.log.gz")
    {
        Ok(appender) => appender,
        Err(err) => panic!("Initializing file log failed: {err}"),
    };

    let log_config = match log4rs::Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log_filter)))
                .build("stdout", Box::new(stdout)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(LevelFilter::Warn)))
                .build("logfile", Box::new(log_file)),
        )
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(LevelFilter::Trace),
        ) {
        Ok(config) => config,
        Err(err) => panic!("Configuring logging failed: {err}"),
    };

    if let Err(err) = log4rs::init_config(log_config) {
        panic!("Initializing logger failed: {err}.");
    }

    if let Err(e) = app(cli_args) {
        log::error!("{e}");
        ::std::process::exit(1);
    }
}
