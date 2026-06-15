// SPDX-License-Identifier: BSD-3-Clause

use std::{env, os::unix::process::CommandExt, process::Command};

use clap::Parser;
use users::get_current_uid;

mod args;
mod common;
mod sudo;

use crate::args::*;
use crate::common::*;

fn main() {
    let cli = Cli::parse();

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let env = env::vars().map(|(key, _)| key).collect();

    let mut cli = cli
        .parse_to_run0_cli(cwd, get_current_uid(), env)
        .finalize()
        .into_iter();

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
