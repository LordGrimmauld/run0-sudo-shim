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

// https://github.com/trifectatechfoundation/sudo-rs/blob/09a5b9acdd462a1606e20f7c241d3b433fbf373a/src/defaults/mod.rs#L72-L78
// https://github.com/sudo-project/sudo/blob/d0a19ef42dd1377e6cbfa0076663406a9ab11920/plugins/sudoers/env.c#L133-L200
// WARNING: This is a Blocklist. Any random piece of software can look at some potentially dangerous environment variable.
// This list may desync with sudo over time. This is a parity issue.
// -E is a VERY bad idea design-wise. So bad, in fact, sudo-rs refuses to accept the flag entirely.
const ENV_DELETE_FIXED: &[&str] = &[
    "IFS",
    "CDPATH",
    "LOCALDOMAIN",
    "RES_OPTIONS",
    "HOSTALIASES",
    "NLSPATH",
    "PATH_LOCALE",
    "SHLIB_PATH",
    "LIBPATH",
    "AUTHSTATE",
    "KRB5_KTNAME",
    "VAR_ACE",
    "USR_ACE",
    "DLC_ACE",
    "TERMINFO",          /* terminfo, exclusive path to terminfo files */
    "TERMINFO_DIRS",     /* terminfo, path(s) to terminfo files */
    "TERMPATH",          /* termcap, path(s) to termcap files */
    "TERMCAP",           /* XXX - only if it starts with '/' */
    "ENV",               /* ksh, file to source before script runs */
    "BASH_ENV",          /* bash, file to source before script runs */
    "PS4",               /* bash, prefix for lines in xtrace mode */
    "GLOBIGNORE",        /* bash, globbing patterns to ignore */
    "BASHOPTS",          /* bash, initial "shopt -s" options */
    "SHELLOPTS",         /* bash, initial "set -o" options */
    "JAVA_TOOL_OPTIONS", /* java, extra command line options */
    "_JAVA_OPTIONS",     /* java, extra command line options (legacy) */
    "CLASSPATH",         /* java, class search path */
    "PERLIO_DEBUG",      /* perl, debugging output file */
    "PERLLIB",           /* perl, search path for modules/includes */
    "PERL5LIB",          /* perl 5, search path for modules/includes */
    "PERL5OPT",          /* perl 5, extra command line options */
    "PERL5DB",           /* perl 5, command used to load debugger */
    "FPATH",             /* ksh, search path for functions */
    "NULLCMD",           /* zsh, command for null file redirection */
    "READNULLCMD",       /* zsh, command for null file redirection */
    "ZDOTDIR",           /* zsh, search path for dot files */
    "TMPPREFIX",         /* zsh, prefix for temporary files */
    "PYTHONHOME",        /* python, module search path */
    "PYTHONPATH",        /* python, search path */
    "PYTHONINSPECT",     /* python, allow inspection */
    "PYTHONUSERBASE",    /* python, per user site-packages directory */
    "PYTHONSTARTUP",     /* python, interactive mode startup script */
    "RUBYLIB",           /* ruby, library load path */
    "RUBYOPT",           /* ruby, extra command line options */
    "NODE_OPTIONS",      /* node.js, extra command line options */
    "NODE_PATH",         /* node.js, module search path */
    "GIT_SSH_COMMAND",   /* git, custom SSH command */
    "GCONV_PATH",        /* glibc generic char set conversion iface */
];

const ENV_DELETE_PREFIX: &[&str] = &[
    "LD_",
    "_RLD",
    "LDR_",
    "KRB5_CONFIG",
    "GIT_CONFIG_", /* git, global configuration override */
    "_",           /* Underscore is a common marker for "internal" env vars*/
];

fn env_var_allowed(env_var: &str) -> bool {
    if ENV_DELETE_FIXED.contains(&env_var) {
        return false;
    }

    if ENV_DELETE_PREFIX
        .iter()
        .any(|deny| env_var.starts_with(deny))
    {
        return false;
    }

    if env_var.contains("=") {
        return false;
    }

    true
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
            eprintln!(
                "run0-sudo-shim: Potentially insecure use of -E or --preserve-env without explicit list of preserved env vars"
            );
            env::vars()
                .map(|(key, _)| key)
                .filter(|e| env_var_allowed(e))
                .collect()
        } else {
            vars
        };

        vars.into_iter()
            .filter(|e| e != "HOME" && e != "USER")
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
