// SPDX-License-Identifier: BSD-3-Clause

mod args;
use std::{env, os::unix::process::CommandExt, process::Command};

use crate::args::Cli;
use clap::Parser;
use users::get_current_uid;

mod common;
mod sudo;
use crate::common::*;

fn main() {
    let cli = Cli::parse();

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let env = env::vars().map(|(key, _)| key).collect();

    let mut cli = ShimResult::finalize(
        sudo::parse_to_run0_cli(cli, cwd, get_current_uid(), env),
        env!("CARGO_PKG_NAME"),
    )
    .into_iter();

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
