// SPDX-License-Identifier: BSD-3-Clause

use std::{ffi::OsString, path::PathBuf};

use clap::{Args, ValueHint};

#[derive(Debug, Args)]
#[clap(name="sudoedit", version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct SudoeditCli {
    /// [IGNORED] use a helper program for password prompting
    #[clap(long, short = 'A', default_value_t = false)]
    pub askpass: bool,

    /// ring bell when prompting
    #[clap(long, short = 'B', default_value_t = false)]
    pub bell: bool,

    /// diverging from sudo, this sets NOFILE limit, achieving similar behavior as sudo explicitly watching and killing file descriptors
    #[clap(long = "close-from", short = 'C')]
    pub file_descriptor_limit: Option<u32>,

    /// change the working directory before running command
    #[clap(long = "chdir", short = 'D')]
    pub working_directory: Option<String>,

    /// run command as the specified group name or ID
    #[clap(long, short)]
    pub group: Option<String>,

    /// [UNSUPPORTED] run command on host (if supported by plugin)
    #[clap(long)]
    pub host: Option<String>,

    /// non-interactive mode, no prompts are used
    #[clap(long, short, default_value_t = false)]
    pub non_interactive: bool,

    /// [IGNORED] use the specified password prompt
    #[clap(long, short)]
    pub prompt: Option<String>,

    /// [UNSUPPORTED] change the root directory before running command
    #[clap(long, short = 'R')]
    pub chroot: Option<String>,

    /// read password from standard input
    #[clap(long, short = 'S', default_value_t = false)]
    pub stdin: bool,

    /// [UNSUPPORTED] terminate command after the specified time limit
    #[clap(long, short = 'T')]
    pub command_timeout: Option<String>,

    /// run command (or edit file) as specified user name or ID
    #[clap(long, short, value_hint = ValueHint::Username)]
    pub user: Option<String>,

    /// an extra argument to pass to run0 (can be specified multiple times)
    #[clap(long = "run0-extra-arg", allow_hyphen_values = true)]
    pub run0_extra_args: Vec<OsString>,

    /// file to be edited
    #[arg(allow_hyphen_values = true, value_hint = ValueHint::FilePath, trailing_var_arg(true), num_args=1..)]
    pub file: Vec<PathBuf>,
}
