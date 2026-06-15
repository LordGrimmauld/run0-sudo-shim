use clap::{Parser, Subcommand};

use crate::{common::Run0Cli, sudo};

#[derive(Debug, Parser)]
#[command(multicall = true, version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// an extra argument to pass to run0 (can be specified multiple times)
    #[clap(long = "run0-extra-arg", allow_hyphen_values = true, global = true)]
    pub run0_extra_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(alias = "run0-sudo-shim")]
    Sudo(sudo::SudoCli),
}

impl Cli {
    pub fn parse_to_run0_cli(
        self,
        cwd: Option<String>,
        current_uid: users::uid_t,
        current_env: Vec<String>,
    ) -> Run0Cli {
        match self.command {
            Commands::Sudo(args) => Run0Cli::new(
                sudo::parse_to_run0_cli(args, cwd, current_uid, current_env, self.run0_extra_args),
                clap::Command::new("sudo"),
            ),
        }
    }
}
