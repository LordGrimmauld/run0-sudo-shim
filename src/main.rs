mod args;
use std::process::{Command, exit};

use crate::args::Cli;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    if cli.edit {
        panic!("`edit` mode is currently unsupported!");
    }

    if cli.list > 0 || cli.other_user.is_some() {
        panic!("`list` mode is currently unsupported!");
    }

    if cli.chroot.is_some() {
        panic!("`chroot` is currently unsupported!");
    }

    if cli.stdin {
        panic!("passwords via `stdin` are currently unsupported!");
    }

    let command = if cli.validate {
        vec![String::from("true")]
    } else {
        cli.command
    };

    let chdir = cli.working_directory.map(|wd| format!("--chdir={}", wd));

    let non_interactive = if cli.non_interactive {
        Some("--no-ask-password")
    } else {
        None
    };

    let status = Command::new("run0")
        .args(chdir.iter())
        .args(non_interactive.iter())
        .args(command.iter())
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    exit(status.code().unwrap_or(0));
}
