// SPDX-License-Identifier: BSD-3-Clause

use std::{fmt::Display, process::exit};

pub static POLKIT_STDIN_AGENT: &str = match option_env!("POLKIT_STDIN_AGENT") {
    Some(x) => x,
    None => "polkit-stdin-agent",
};

pub static RUN0_CMD: &str = match option_env!("RUN0") {
    Some(x) => x,
    None => "run0",
};

pub static TRUE_CMD: &str = match option_env!("TRUE") {
    Some(x) => x,
    None => "true",
};

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

#[derive(Debug)]
pub struct ShimResult {
    pub cli: Vec<String>,
    stderr: String,
    stdout: String,
}

impl ShimResult {
    pub fn new() -> Self {
        Self {
            cli: Vec::new(),
            stderr: String::new(),
            stdout: String::new(),
        }
    }

    // CAN EXIT(1)
    pub fn finalize(
        res: Result<ShimResult, Error>,
        argv0: impl Into<clap::builder::Str>,
    ) -> Vec<String> {
        let res = match res {
            Ok(res) => res,
            Err(e) => match e {
                Error::PrintHelp => {
                    clap::Command::new(argv0).print_help().ok();
                    exit(1);
                }
                _ => die(&format!("{}", e)),
            },
        };
        eprintln!("{}", res.stderr);
        println!("{}", res.stdout);
        res.cli
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
