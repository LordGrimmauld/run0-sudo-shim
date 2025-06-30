mod args;
use std::{
    env,
    os::unix::process::CommandExt,
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

    let chdir = cli.working_directory.map(|wd| format!("--chdir={wd}"));

    let non_interactive = if cli.non_interactive {
        Some("--no-ask-password")
    } else {
        None
    };

    let group = cli
        .group
        .map(|g| format!("--group={}", g.trim_start_matches('#')));
    let user = cli
        .user
        .map(|u| format!("--user={}", u.trim_start_matches('#')));

    let env_flags = if let Some(vars) = cli.preserve_env {
        let vars = if vars.is_empty() {
            env::vars().map(|(key, _)| key).collect()
        } else {
            vars
        };

        vars.iter()
            .filter(|e| !(cli.set_home && *e == "HOME"))
            .map(|e| format!("--setenv={e}"))
            .collect()
    } else {
        Vec::new()
    };

    let nofile = cli
        .file_descriptor_limit
        .map(|limit_nofile| format!("--property=LimitNOFILE={limit_nofile}"));

    if command.is_empty() && !cli.login {
        let mut cmd = clap::Command::new(env!("CARGO_PKG_NAME"));
        cmd.print_help().ok();
        exit(0);
    }

    if cli.bell && !cli.non_interactive {
        print!("\x07");
    }

    let error = Command::new("run0")
        .args(chdir.iter())
        .args(non_interactive.iter())
        .args(group.iter())
        .args(user.iter())
        .args(nofile.iter())
        .args(env_flags)
        .args(command)
        .exec();

    panic!("{}", error);
}
