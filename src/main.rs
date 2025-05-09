mod args;
use std::{
    env,
    process::{Command, exit},
};

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

    let group = cli.group.map(|g| format!("--group={}", g));
    let user = cli.user.map(|u| format!("--user={}", u));

    let env_flags = if let Some(vars) = cli.preserve_env {
        let vars = if vars.is_empty() {
            env::vars().map(|(key, _)| key).collect()
        } else {
            vars
        };

        vars.iter()
            .filter(|e| !(cli.set_home && *e == "HOME"))
            .map(|e| format!("--setenv={}", e))
            .collect()
    } else {
        Vec::new()
    };

    if cli.bell && !cli.non_interactive {
        print!("\x07");
    }

    let status = Command::new("run0")
        .args(chdir.iter())
        .args(non_interactive.iter())
        .args(group.iter())
        .args(user.iter())
        .args(env_flags)
        .args(command)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    exit(status.code().unwrap_or(0));
}
