use clap::{Parser, Subcommand};

use crate::sudo;

#[derive(Debug, Parser)]
#[command(multicall = true, version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// an extra argument to pass to run0 (can be specified multiple times)
    #[clap(long = "run0-extra-arg", allow_hyphen_values = true, global = true)]
    pub run0_extra_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(alias = "run0-sudo-shim")]
    Sudo(sudo::SudoCli),
}
