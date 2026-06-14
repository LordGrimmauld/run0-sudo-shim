// SPDX-License-Identifier: BSD-3-Clause

use std::{env, os::unix::process::CommandExt, process::Command};

use clap::Parser;
use users::get_current_uid;

mod common;
mod sudo;
use crate::common::*;

fn main() {
    let cwd = env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let env = env::vars().map(|(key, _)| key).collect();

    let cmd = env::args_os()
        .next()
        .and_then(|arg0| {
            std::path::Path::new(&arg0)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| die("failed to read argv[0]"));

    #[allow(clippy::match_single_binding)]
    let mut cli = match cmd.as_str() {
        // "pkexec" => panic!("not implemented"),
        _ => ShimResult::finalize(
            sudo::parse_to_run0_cli(sudo::Cli::parse(), cwd, get_current_uid(), env),
            "sudo",
        ),
    }
    .into_iter();

    let program = cli.next().unwrap_or_else(|| die("unable to construct cli"));

    let error = Command::new(program).args(cli).exec();

    die(&format!("failed to execute run0: {error}"));
}
