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

static RUN0_CMD: &str = match option_env!("RUN0") {
    Some(x) => x,
    None => "run0",
};

static TRUE_CMD: &str = match option_env!("TRUE") {
    Some(x) => x,
    None => "true",
};

fn die(msg: &str) -> ! {
    eprintln!("run0-sudo-shim: {msg}");
    exit(1)
}

fn main() {
    let cli = Cli::parse();

    if cli.edit {
        die("`edit` mode is currently unsupported!");
    }

    if cli.list > 0 || cli.other_user.is_some() {
        die("`list` mode is currently unsupported!");
    }

    if cli.chroot.is_some() {
        die("`chroot` is currently unsupported!");
    }

    if cli.stdin {
        die("passwords via `stdin` are currently unsupported!");
    }

    if cli.remove_timestamp || cli.reset_timestamp {
        // potential solution: call RevokeTemporaryAuthorizations on org.freedesktop.PolicyKit1.Authority dbus
        die("removing or resetting authentication timestamps is currently unsupported")
    }

    if cli.host.is_some() {
        die("`host` is currently unsupported!")
    }

    if cli.preserve_groups {
        die("`preserve-groups` is currently unsupported!")
    }

    if cli.background {
        die("`background` is currently unsupported!")
    }

    if cli.askpass {
        eprintln!("run0-sudo-shim: --askpass is currently ignored");
    }

    if cli.prompt.is_some() {
        eprintln!("run0-sudo-shim: --prompt is currently ignored");
    }

    let command = if cli.validate {
        vec![String::from(TRUE_CMD)]
    } else {
        cli.command
    };

    let shell = if cli.shell || cli.login {
        Some("--via-shell")
    } else {
        None
    };

    let chdir = cli
        .working_directory
        .or(if cli.login {
            Some(String::from("~"))
        } else {
            env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .ok()
        })
        .map(|wd| format!("--chdir={wd}"));

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
        .or(if group.is_some() {
            Some(get_current_uid().to_string())
        } else {
            None
        })
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

    let runtime_max = cli
        .command_timeout
        .map(|timeout_secs| format!("--property=RuntimeMaxSec={timeout_secs}"));

    let run0_extra_args = cli.run0_extra_args;

    if command.is_empty() && shell.is_none() {
        let mut cmd = clap::Command::new(env!("CARGO_PKG_NAME"));
        cmd.print_help().ok();
        exit(1);
    }

    if cli.bell && !cli.non_interactive {
        print!("\x07");
    }

    let error = Command::new(RUN0_CMD)
        .args(shell.iter())
        .args(chdir.iter())
        .args(non_interactive.iter())
        .args(group.iter())
        .args(user.iter())
        .args(nofile.iter())
        .args(runtime_max.iter())
        .args(env_flags)
        .args(run0_extra_args.iter())
        .arg("--")
        .args(command)
        .exec();

    die(&format!("failed to execute run0: {error}"));
}
