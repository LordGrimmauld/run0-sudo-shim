mod args;

use crate::args::Cli;
use clap::Parser;

use nix::{
    fcntl::{AT_FDCWD, OFlag, OpenHow, ResolveFlag, openat2},
    libc::{self},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::stat::Mode,
};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{self},
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::{MetadataExt, OpenOptionsExt, fchown},
    },
    path::Path,
};

fn open_dir(path: &Path) -> io::Result<OwnedFd> {
    let dir_str: &OsStr = if nix::NixPath::is_empty(path) {
        // FIXME: unstable feature to be replaced by std
        eprintln!("received empty path");
        &OsString::from(".")
    } else {
        path.as_os_str()
    };
    let fd = openat2(
        AT_FDCWD,
        dir_str,
        OpenHow::new()
            .flags(OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC)
            .mode(Mode::empty())
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS),
    )?;
    Ok(fd)
}

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

fn copy_into_new_file(
    src_dir: &OwnedFd,
    src_filename: &OsStr,
    dst_dir: &OwnedFd,
    dst_filename: &OsStr,
    owner_uid: Option<u32>,
) -> io::Result<()> {
    let dst_fd = openat2(
        dst_dir,
        dst_filename,
        OpenHow::new()
            .flags(
                OFlag::O_WRONLY
                    | OFlag::O_CREAT
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_EXCL
                    | OFlag::O_CLOEXEC,
            )
            .mode(Mode::S_IRUSR | Mode::S_IWUSR)
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS | ResolveFlag::RESOLVE_BENEATH),
    )?;

    let mut dst_file = File::from(dst_fd);

    let src_fd = openat2(
        src_dir,
        src_filename,
        OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS | ResolveFlag::RESOLVE_BENEATH),
    );

    match src_fd {
        Ok(src_fd) => {
            let mut src_file = File::from(src_fd);
            io::copy(&mut src_file, &mut dst_file)?;
        }
        Err(errno) => match errno {
            nix::errno::Errno::ENOENT => {
                // no original file existing is fine
            }
            _ => {
                return Err(errno.into());
            }
        },
    }

    dst_file.sync_all()?;

    fchown(dst_file, owner_uid, None)?;

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

    let privileged_dir = open_dir(cli.file.parent().expect("file has parent"))?;
    let privileged_filename = cli.file.file_name().expect("file has name");
    let temp_dir = open_dir(cli.tmp_path.parent().expect("file has parent"))?;
    let temp_filename = cli.tmp_path.file_name().expect("file has name");

    let pidfd = pidfd_open(cli.editor_pid)?; // fail early if editor stopped existing

    copy_into_new_file(
        &privileged_dir,
        privileged_filename,
        &temp_dir,
        temp_filename,
        uid_from_pid(cli.editor_pid),
    )?;

    let mut fds = [PollFd::new(pidfd.as_fd(), PollFlags::POLLIN)];
    poll(&mut fds, PollTimeout::NONE)?;

    println!("editor terminated");

    copy_into_privileged(&cli.file, &cli.tmp_path, cli.editor_pid)?;

    Ok(())
}
