// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(name="run0-edit-daemon", version=env!("CARGO_PKG_VERSION"),about=env!("CARGO_PKG_DESCRIPTION"), author=env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    /// PID of the editor process. Used to determine termination and audit rules.
    #[clap(long)]
    pub editor_pid: i32,

    /// file to edit
    #[clap(long)]
    pub file: PathBuf,

    /// file path to copy to
    #[clap(long)]
    pub tmp_path: PathBuf,
}
