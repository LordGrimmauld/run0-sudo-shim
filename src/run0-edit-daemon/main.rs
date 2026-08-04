mod args;

use crate::args::Cli;
use clap::Parser;

use nix::{
    libc::{self},
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self},
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::{MetadataExt, OpenOptionsExt, fchown},
    },
    path::Path,
};

fn uid_from_pid(pid: i32) -> Option<u32> {
    let metadata = fs::metadata(format!("/proc/{pid}")).ok()?;
    Some(metadata.uid())
}

fn pidfd_open(pid: libc::pid_t) -> nix::Result<OwnedFd> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };

    if fd < 0 {
        return Err(nix::Error::last());
    }

    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

fn copy_into_new_file(src: &Path, dst: &Path, owner_uid: Option<u32>) -> io::Result<()> {
    let mut output = OpenOptions::new()
        .write(true) // O_WRONLY because no read(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dst)?;

    match File::open(src) {
        Ok(mut input) => {
            io::copy(&mut input, &mut output)?;
        }
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                // no original file existing is fine
            }
            _ => {
                return Err(e);
            }
        },
    }

    output.sync_all()?;

    fchown(output, owner_uid, None).map_err(io::Error::other)?;

    Ok(())
}

fn copy_into_privileged(dst: &Path, src: &Path, editor_pid: i32) -> io::Result<()> {
    let tmp = dst
        .with_added_extension("tmp")
        .with_added_extension(editor_pid.to_string());
    let mut input = File::open(src)?;

    // TODO: xattrs
    let (uid, gid, mode) = match fs::symlink_metadata(dst) {
        Ok(meta) => (meta.uid(), meta.gid(), meta.mode()),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => (0, 0, 0o644),
            _ => {
                return Err(e);
            }
        },
    };

    let mut output_tmp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)?;

    io::copy(&mut input, &mut output_tmp)?;

    fchown(output_tmp.as_fd(), Some(uid), Some(gid))?;

    output_tmp.sync_all()?;
    drop(output_tmp);

    fs::rename(&tmp, dst)?;
    fs::remove_file(src)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let pidfd = pidfd_open(cli.editor_pid)?; // fail early if editor stopped existing
    copy_into_new_file(&cli.file, &cli.tmp_path, uid_from_pid(cli.editor_pid))?;

    let mut fds = [PollFd::new(pidfd.as_fd(), PollFlags::POLLIN)];
    poll(&mut fds, PollTimeout::NONE)?;

    println!("editor terminated");

    copy_into_privileged(&cli.file, &cli.tmp_path, cli.editor_pid)?;

    Ok(())
}
