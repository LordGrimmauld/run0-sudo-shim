// SPDX-License-Identifier: BSD-3-Clause

use std::{ffi::OsString, fmt::Display, process::exit};

pub fn die(msg: &str) -> ! {
    eprintln!("run0-sudo-shim: {msg}");
    exit(1)
}

#[derive(PartialEq, Eq, Debug)]
#[allow(dead_code)]
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

pub struct ShimResult {
    pub cli: Vec<OsString>,
    pub post_run0_hook: Option<PostRunClosure>,
    stderr: String,
    stdout: String,
}

impl std::fmt::Debug for ShimResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShimResult")
            .field("cli", &self.cli)
            .field("post_run0_hook", &self.post_run0_hook.is_some())
            .field("stderr", &self.stderr)
            .field("stdout", &self.stdout)
            .finish()
    }
}

impl PartialEq for ShimResult {
    fn eq(&self, other: &Self) -> bool {
        self.cli == other.cli && self.stderr == other.stderr && self.stdout == other.stdout
    }
}

impl ShimResult {
    pub fn new() -> Self {
        Self {
            cli: Vec::new(),
            stderr: String::new(),
            stdout: String::new(),
            post_run0_hook: None,
        }
    }

    #[cfg(test)]
    pub fn ok_from(cli: Vec<OsString>) -> Result<Self, Error> {
        Ok(Self {
            cli,
            stderr: String::new(),
            stdout: String::new(),
            post_run0_hook: None,
        })
    }
    #[cfg(test)]
    pub fn get_stderr(&self) -> &str {
        &self.stderr
    }

    #[cfg(test)]
    pub fn get_stdout(&self) -> &str {
        &self.stdout
    }

    pub fn push_stderr(&mut self, line: impl std::fmt::Display) {
        use std::fmt::Write;
        write!(self.stderr, "{line}").unwrap();
    }
    pub fn push_stdout(&mut self, line: impl std::fmt::Display) {
        use std::fmt::Write;
        write!(self.stderr, "{line}").unwrap();
    }
}

pub struct Run0Cli {
    pub res: Result<ShimResult, Error>,
    cmd: clap::Command,
}

type PostRunClosure = Box<dyn FnOnce()>;

impl Run0Cli {
    pub fn new(res: Result<ShimResult, Error>, cmd: clap::Command) -> Self {
        Self { res, cmd }
    }

    // CAN EXIT(1)
    pub fn finalize(mut self) -> (Vec<OsString>, Option<PostRunClosure>) {
        let res = match self.res {
            Ok(res) => res,
            Err(e) => match e {
                Error::PrintHelp => {
                    self.cmd.print_help().ok();
                    exit(1);
                }
                _ => die(&format!("{}", e)),
            },
        };
        if !res.stderr.is_empty() {
            eprintln!("{}", res.stderr);
        }
        if !res.stdout.is_empty() {
            println!("{}", res.stdout);
        }
        (res.cli, res.post_run0_hook)
    }
}
