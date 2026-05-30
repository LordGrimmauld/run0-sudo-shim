// SPDX-License-Identifier: BSD-3-Clause

mod args;
use std::{
    env,
    os::unix::process::CommandExt,
    process::{Command, exit},
};

use crate::args::Cli;
use clap::Parser;
use users::get_current_uid;

mod builder;
use crate::builder::*;

pub fn die(msg: &str) -> ! {
    eprintln!("run0-sudo-shim: {msg}");
    exit(1)
}

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

    let mut cli = match parse_to_run0_cli(cli, cwd, get_current_uid()) {
        Ok(cli) => cli.into_iter(),
        Err(e) => match e {
            Error::PrintHelp => {
                let mut cmd = clap::Command::new(env!("CARGO_PKG_NAME"));
                cmd.print_help().ok();
                exit(1);
            }
            _ => die(&format!("{}", e)),
        },
    };

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
