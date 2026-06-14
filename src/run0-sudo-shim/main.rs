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

    let mut cli = match cli.command {
        Commands::Sudo(args) => {
            ShimResult::finalize(
                sudo::parse_to_run0_cli(args, cwd, get_current_uid(), env, cli.run0_extra_args),
                "sudo",
            )
        }
        .into_iter(),
    };

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
