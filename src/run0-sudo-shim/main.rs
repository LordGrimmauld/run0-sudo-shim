// SPDX-License-Identifier: BSD-3-Clause

mod args;
use std::{
    env,
    fmt::Display,
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

#[derive(PartialEq, Eq, Debug)]
pub enum Error {
    Unsupported(String),
    UnknownUser(String),
    UnknownGroup(String),
    PrintHelp,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(feat) => {
                f.write_fmt(format_args!("{} is currently unsupported", feat))
            }
            Self::PrintHelp => f.write_str(""), // FIXME
            Error::UnknownUser(u) => f.write_fmt(format_args!("unknown user: {u}")),
            Error::UnknownGroup(g) => f.write_fmt(format_args!("unknown group: {g}")),
        }
    }
}

impl std::error::Error for Error {}

fn parse_to_run0_cli(cli: Cli) -> Result<Vec<String>, Error> {
    let mut buf: Vec<String> = Vec::new();
    if cli.edit {
        return Err(Error::Unsupported(String::from("--edit")));
    }

    if cli.list > 0 || cli.other_user.is_some() {
        return Err(Error::Unsupported(String::from("list mode")));
    }

    if cli.chroot.is_some() {
        return Err(Error::Unsupported(String::from("--chroot")));
    }

    if cli.stdin {
        return Err(Error::Unsupported(String::from("--stdin")));
    }

    if cli.remove_timestamp || cli.reset_timestamp {
        // potential solution: call RevokeTemporaryAuthorizations on org.freedesktop.PolicyKit1.Authority dbus
        return Err(Error::Unsupported(String::from(
            "removing or resetting authentication timestamps",
        )));
    }

    if cli.host.is_some() {
        return Err(Error::Unsupported(String::from("--host")));
    }

    if cli.preserve_groups {
        return Err(Error::Unsupported(String::from("--preserve-groups")));
    }

    if cli.background {
        return Err(Error::Unsupported(String::from("--background")));
    }

    buf.push(String::from(RUN0_CMD));

    if cli.shell || cli.login {
        buf.push(String::from("--via-shell"));
    }

    if let Some(work_dir) = cli.working_directory.or(if cli.login {
        Some(String::from("~"))
    } else {
        // FIXME: impure
        env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .ok()
    }) {
        buf.push(format!("--chdir={work_dir}"));
    }

    if cli.non_interactive {
        buf.push(String::from("--no-ask-password"))
    }

    if let Some(user) = cli.user {
        // FIXME: handle numerics safely
        buf.push(format!("--user={}", user.trim_start_matches('#')))
    } else if cli.group.is_some() {
        // FIXME: impure
        buf.push(format!("--user={}", get_current_uid()))
    }

    if let Some(group) = cli.group {
        // FIXME: handle numerics safely
        buf.push(format!("--group={}", group.trim_start_matches('#')))
    }

    if let Some(vars) = cli.preserve_env {
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
        buf.extend(
            vars.into_iter()
                .filter(|e| e != "HOME" && e != "USER")
                .map(|e| format!("--setenv={e}")),
        );
    }

    if let Some(limit_nofile) = cli.file_descriptor_limit {
        buf.push(format!("--property=LimitNOFILE={limit_nofile}"));
    }

    if let Some(timeout_secs) = cli.command_timeout {
        buf.push(format!("--property=RuntimeMaxSec={timeout_secs}"));
    }

    buf.extend(cli.run0_extra_args);
    buf.push(String::from("--"));

    if cli.validate {
        buf.push(String::from(TRUE_CMD));
    } else if !cli.command.is_empty() {
        buf.extend(cli.command);
    } else if !(cli.shell || cli.login) {
        return Err(Error::PrintHelp);
    }

    return Ok(buf);
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

    let mut cli = match parse_to_run0_cli(cli) {
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
