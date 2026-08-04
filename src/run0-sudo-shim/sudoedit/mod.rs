// SPDX-License-Identifier: BSD-3-Clause

use std::{
    env,
    ffi::OsString,
    fs,
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
};

use users::uid_t;

use crate::common::*;
use crate::external_programs::{POLKIT_STDIN_AGENT, RUN0_EDIT_DAEMON, SYSTEMD_RUN_CMD};

mod args;
pub use args::SudoeditCli;

#[cfg(not(test))]
use rand::{rand_core::TryRng, rngs::SysRng};

fn post_run0_hook(path: &Path, current_uid: u32) {
    // Wait for the daemon to create the file and chwon it (signaling it is ready).
    loop {
        if let Ok(metadata) = fs::metadata(path)
            && metadata.uid() == current_uid
        {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let editor = env::var("SUDO_EDITOR")
        .or(env::var("VISUAL"))
        .or(env::var("EDITOR"))
        .unwrap_or("nano".into());

    let err = Command::new(editor).arg(path).exec();

    eprintln!("could not start editor: {err}");
    std::process::exit(1);
}

#[cfg(not(test))]
fn generate_file_path() -> PathBuf {
    use std::path::PathBuf;

    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR not set");

    let mut bytes = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("failed to obtain random");

    let name = format!("run0-edit-{:x}", u128::from_ne_bytes(bytes));

    PathBuf::from(runtime_dir).join(name)
}

#[cfg(test)]
fn generate_file_path() -> PathBuf {
    let runtime_dir = "/run/user/1000";
    let bytes = [0u8; 16]; // deterministic in tests
    let name = format!("run0-edit-{:x}", u128::from_ne_bytes(bytes));
    PathBuf::from(runtime_dir).join(name)
}

pub fn parse_to_run0_cli(
    cli: SudoeditCli,
    cwd: Option<String>,
    current_pid: u32,
    current_uid: uid_t,
) -> Result<ShimResult, Error> {
    if cli.chroot.is_some() {
        return Err(Error::Unsupported(String::from("--chroot")));
    }

    if cli.host.is_some() {
        // potential solution: raw systemd-run with `--host`
        return Err(Error::Unsupported(String::from("--host")));
    }

    if cli.command_timeout.is_some() {
        return Err(Error::Unsupported(String::from("--command-timeout")));
    }

    if cli.file.len() > 1 {
        return Err(Error::Unsupported(String::from("passing multiple files")));
    }

    let file = if let Some(file) = cli.file.first() {
        file
    } else {
        return Err(Error::PrintHelp);
    };

    let mut buf = ShimResult::new();

    if cli.askpass {
        buf.push_stderr("run0-sudo-shim: --askpass is currently ignored");
    }

    if cli.prompt.is_some() {
        buf.push_stderr("run0-sudo-shim: --prompt is currently ignored");
    }

    if cli.bell && !cli.non_interactive {
        buf.push_stdout("\x07");
    }

    if cli.stdin {
        buf.cli.push(OsString::from(POLKIT_STDIN_AGENT));
        buf.cli.push(OsString::from("--password-fd=0"));
        buf.cli.push(OsString::from("--"));
    }

    buf.cli.push(OsString::from(SYSTEMD_RUN_CMD));
    buf.cli.push(OsString::from("--collect"));
    buf.cli.push(OsString::from("--no-pager"));
    buf.cli.push(OsString::from("--no-block"));

    if let Some(work_dir) = cli.working_directory.or(cwd) {
        buf.cli
            .push(OsString::from(format!("--working-directory={work_dir}")));
    }

    if cli.non_interactive {
        buf.cli.push(OsString::from("--no-ask-password"))
    }

    if let Some(user) = cli.user {
        // FIXME: handle numerics safely
        buf.cli.push(OsString::from(format!(
            "--user={}",
            user.trim_start_matches('#')
        )))
    } else if cli.group.is_some() {
        buf.cli
            .push(OsString::from(format!("--user={}", current_uid)))
    }

    if let Some(group) = cli.group {
        // FIXME: handle numerics safely
        buf.cli.push(OsString::from(format!(
            "--group={}",
            group.trim_start_matches('#')
        )))
    }

    if let Some(limit_nofile) = cli.file_descriptor_limit {
        buf.cli.push(OsString::from(format!(
            "--property=LimitNOFILE={limit_nofile}"
        )));
    }

    if let Some(timeout_secs) = cli.command_timeout {
        buf.cli.push(OsString::from(format!(
            "--property=RuntimeMaxSec={timeout_secs}"
        )));
    }

    let temp_file_path = generate_file_path();
    buf.cli.extend(cli.run0_extra_args);
    buf.cli.push(OsString::from("--"));
    buf.cli.push(OsString::from(RUN0_EDIT_DAEMON));
    buf.cli.push(OsString::from("--file"));
    buf.cli.push(file.into());
    buf.cli.push(OsString::from("--tmp-path"));
    buf.cli.push(temp_file_path.clone().into_os_string());
    buf.cli.push(OsString::from("--editor-pid"));
    buf.cli.push(current_pid.to_string().into());
    buf.post_run0_hook = Some(Box::new(move || {
        post_run0_hook(&temp_file_path, current_uid)
    }));

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::args::Cli;

    #[test]
    fn test_simple_file() {
        let cli = Cli::parse_from(["sudoedit", "--", "/root/file"]);
        assert!(matches!(cli.command, crate::Commands::Sudoedit(_)));
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }
    #[test]
    fn test_bare() {
        let cli = Cli::parse_from(["sudoedit"]);
        assert!(matches!(cli.command, crate::Commands::Sudoedit(_)));
        let crate::Commands::Sudoedit(sudo_cli) = &cli.command else {
            unreachable!()
        };
        assert!(sudo_cli.file.is_empty());
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(build_result, Err(Error::PrintHelp));
    }
    #[test]
    fn test_chdir() {
        let cli = Cli::parse_from(["sudoedit", "-D", "/foo", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--working-directory=/foo"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_stdin() {
        let cli = Cli::parse_from(["sudoedit", "--stdin", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(POLKIT_STDIN_AGENT),
                OsString::from("--password-fd=0"),
                OsString::from("--"),
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_close_from() {
        let cli = Cli::parse_from(["sudoedit", "-C", "1000", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--property=LimitNOFILE=1000"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_group() {
        let cli = Cli::parse_from(["sudoedit", "-g", "dialout", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--user=1000"), // -g should maintain spawning user
                OsString::from("--group=dialout"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_group_and_user() {
        let cli = Cli::parse_from(["sudoedit", "-g", "dialout", "-u", "root", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--user=root"),
                OsString::from("--group=dialout"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_numeric_user() {
        let cli = Cli::parse_from(["sudoedit", "-u", "#0", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--user=0"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_named_user() {
        let cli = Cli::parse_from(["sudoedit", "-u", "root", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--user=root"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_non_interactive() {
        let cli = Cli::parse_from(["sudoedit", "-n", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--no-ask-password"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }

    #[test]
    fn test_extra_arg() {
        let cli = Cli::parse_from(["sudoedit", "--run0-extra-arg=--background=42", "/root/file"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                OsString::from(SYSTEMD_RUN_CMD),
                OsString::from("--collect"),
                OsString::from("--no-pager"),
                OsString::from("--no-block"),
                OsString::from("--background=42"),
                OsString::from("--"),
                OsString::from(RUN0_EDIT_DAEMON),
                OsString::from("--file"),
                OsString::from("/root/file"),
                OsString::from("--tmp-path"),
                OsString::from("/run/user/1000/run0-edit-0"),
                OsString::from("--editor-pid"),
                OsString::from("0"),
            ])
        );
    }
}

#[cfg(test)]
mod unsupported {
    use clap::Parser;

    use super::*;
    use crate::args::Cli;

    #[test]
    fn test_host_unsupported() {
        let cli = Cli::parse_from(["sudoedit", "--host", "foo", "bar"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--host")))
        );
    }

    #[test]
    fn test_timeout_unsupported() {
        let cli = Cli::parse_from(["sudoedit", "--command-timeout", "0", "bar"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--command-timeout")))
        );
    }

    #[test]
    fn test_chroot_unsupported() {
        let cli = Cli::parse_from(["sudoedit", "-R", "/", "bar"]);
        let build_result = cli.parse_to_run0_cli(None, 1000, vec![], 0).res;
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--chroot")))
        );
    }
}
