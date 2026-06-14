// SPDX-License-Identifier: BSD-3-Clause

mod args;
use std::{env, os::unix::process::CommandExt, process::Command};

use crate::args::Cli;
use clap::Parser;
use users::get_current_uid;

mod builder;
use crate::builder::*;

fn main() {
    let cli = Cli::parse();

    if cli.askpass {
        eprintln!("run0-sudo-shim: --askpass is currently ignored");
    }

    if cli.prompt.is_some() {
        eprintln!("run0-sudo-shim: --prompt is currently ignored");
    }

    if cli.bell && !cli.non_interactive {
        print!("\x07");
    }

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let env = env::vars().map(|(key, _)| key).collect();

    let mut cli = ShimResult::finalize(
        parse_to_run0_cli(cli, cwd, get_current_uid(), env),
        env!("CARGO_PKG_NAME"),
    )
    .into_iter();

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
