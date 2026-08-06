// SPDX-License-Identifier: BSD-3-Clause

use std::{env, os::unix::process::CommandExt, process::Command};

use clap::Parser;
use users::get_current_uid;

mod args;
mod common;

mod sudo;

#[cfg(feature = "sudoedit")]
mod sudoedit;

use crate::args::*;
use crate::common::*;

impl Cli {
    pub fn parse_to_run0_cli(
        self,
        cwd: Option<String>,
        current_uid: users::uid_t,
        current_env: Vec<String>,
        current_pid: u32,
    ) -> Run0Cli {
        match self.command {
            crate::Commands::Sudo(args) => Run0Cli::new(
                sudo::parse_to_run0_cli(args, cwd, current_pid, current_uid, current_env),
                clap::Command::new("sudo"),
            ),
            #[cfg(feature = "sudoedit")]
            crate::Commands::Sudoedit(args) => Run0Cli::new(
                sudoedit::parse_to_run0_cli(args, cwd, current_pid, current_uid),
                clap::Command::new("sudoedit"),
            ),
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let env = env::vars().map(|(key, _)| key).collect();

    let parsed = cli.parse_to_run0_cli(cwd, get_current_uid(), env, std::process::id());
    let (cli, post_run0_hook) = parsed.finalize();

    let program = cli
        .first()
        .unwrap_or_else(|| die("unable to construct cli"));

    if let Some(hook) = post_run0_hook {
        if let Err(error) = Command::new(program).args(cli).spawn() {
            die(&format!("failed to exec run0: {error}"));
        }
        hook();
    } else {
        let error = Command::new(program).args(cli).exec();
        die(&format!("failed to exec run0: {error}"));
    }
}
