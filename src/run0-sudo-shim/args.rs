// SPDX-License-Identifier: BSD-3-Clause

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(multicall = true, version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    #[command(alias = "run0-sudo-shim")]
    Sudo(crate::sudo::SudoCli),
}
