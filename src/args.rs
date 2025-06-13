// SPDX-License-Identifier: GPL-3.0-only

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[clap(name=env!("CARGO_PKG_NAME"), version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    /// [IGNORED] use a helper program for password prompting
    #[clap(long, short = 'A', default_value_t = false)]
    pub askpass: bool,

    /// [IGNORED] run command in the background
    #[clap(long, short, default_value_t = false)]
    pub background: bool,

    /// ring bell when prompting
    #[clap(long, short = 'B', default_value_t = false)]
    pub bell: bool,

    /// diverging from sudo, this sets NOFILE limit, achieving similar behavior as sudo explicitly watching and killing file descriptors
    #[clap(long = "close-from", short = 'C')]
    pub file_descriptor_limit: Option<u32>,

    /// change the working directory before running command
    #[clap(long = "chdir", short = 'D')]
    pub working_directory: Option<String>,

    /// preserve user environment when running command
    #[clap(long, short = 'E', value_delimiter(','), num_args(0..), require_equals(true))]
    pub preserve_env: Option<Vec<String>>,

    /// [UNSUPPORTED] edit files instead of running a command
    #[clap(long, short, default_value_t = false)]
    pub edit: bool,

    /// run command as the specified group name or ID
    #[clap(long, short)]
    pub group: Option<String>,

    /// set HOME variable to target user's home dir
    #[clap(long, short = 'H', default_value_t = false)]
    pub set_home: bool,

    /// [IGNORED] run command on host (if supported by plugin)
    #[clap(long, default_value_t = false)]
    pub host: bool,

    /// run login shell as the target user; a command may also be specified
    #[clap(long, short = 'i', default_value_t = false)]
    pub login: bool,

    /// [IGNORED] remove timestamp file completely
    #[clap(long, short = 'K', default_value_t = false)]
    pub remove_timestamp: bool,

    /// [IGNORED] invalidate timestamp file
    #[clap(long, short = 'k', default_value_t = false)]
    pub reset_timestamp: bool,

    /// [UNSUPPORTED] list user's privileges or check a specific command; use twice for longer format
    #[clap(long, short = 'l', action = ArgAction::Count)]
    pub list: u8,

    /// non-interactive mode, no prompts are used
    #[clap(long, short, default_value_t = false)]
    pub non_interactive: bool,

    /// [IGNORED] preserve group vector instead of setting to target's
    #[clap(long, short = 'P', default_value_t = false)]
    pub preserve_groups: bool,

    /// [IGNORED] use the specified password prompt
    #[clap(long, short)]
    pub prompt: Option<String>,

    /// [UNSUPPORTED] change the root directory before running command
    #[clap(long, short = 'R')]
    pub chroot: Option<String>,

    /// [UNSUPPORTED] read password from standard input
    #[clap(long, short = 'S', default_value_t = false)]
    pub stdin: bool,

    /// [IGNORED] run shell as the target user; a command may also be specified
    #[clap(long, short, default_value_t = false)]
    pub shell: bool,

    /// [IGNORED] terminate command after the specified time limit
    #[clap(long, short = 'T')]
    pub command_timeout: Option<String>,

    /// [UNSUPPORTED] in list mode, display privileges for user
    #[clap(long, short = 'U')]
    pub other_user: Option<String>,

    /// run command (or edit file) as specified user name or ID
    #[clap(long, short)]
    pub user: Option<String>,

    /// validate a root login
    #[clap(long, short = 'v', default_value_t = false)]
    pub validate: bool,

    /// command to be executed
    #[arg(last(false), allow_hyphen_values = true)]
    pub command: Vec<String>,
}
