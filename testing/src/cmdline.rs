// SPDX-License-Identifier: MIT
use anyhow::Result;

/// Run a command line given by one line of text
pub fn exec_cmd_line<P>(f: fn(P) -> Result<()>, cmd_line: Vec<&str>) -> Result<()>
where
    P: clap::Parser,
{
    match f(P::parse_from(cmd_line)) {
        Err(err) => {
            eprintln!("{err}");
            Err(err)
        }
        ok => ok,
    }
}
