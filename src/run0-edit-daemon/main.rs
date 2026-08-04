mod args;

use crate::args::Cli;
use clap::Parser;

use nix::{
    fcntl::{AT_FDCWD, AtFlags, OFlag, OpenHow, ResolveFlag, openat2, renameat},
    libc::{self, dev_t, ino_t},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::stat::{Mode, fchmod, fstat, fstatat},
    unistd::{fsync, linkat, unlinkat},
};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io,
    os::{
        fd::{AsFd, FromRawFd, OwnedFd},
        unix::fs::{MetadataExt, fchown},
    },
    path::Path,
};

fn open_dir(path: &Path) -> io::Result<OwnedFd> {
    let dir_str: &OsStr = if nix::NixPath::is_empty(path) {
        // FIXME: unstable feature to be replaced by std
        &OsString::from(".")
    } else {
        path.as_os_str()
    };
    let fd = openat2(
        AT_FDCWD,
        dir_str,
        OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC) // later unlinkat imposes O_RDONLY instead of O_PATH
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

type FileDiskInfo = (ino_t, dev_t);

fn copy_into_new_file(
    src_dir: &OwnedFd,
    src_filename: &OsStr,
    dst_dir: &OwnedFd,
    dst_filename: &OsStr,
    owner_uid: Option<u32>,
) -> io::Result<(Option<FileDiskInfo>, FileDiskInfo)> {
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

    let dst_stat = fstat(&dst_fd)?;
    let dst_info: FileDiskInfo = (dst_stat.st_ino, dst_stat.st_dev);

    let mut dst_file = File::from(dst_fd);

    let src_fd = openat2(
        src_dir,
        src_filename,
        OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS | ResolveFlag::RESOLVE_BENEATH),
    );

    let src_info = match src_fd {
        Ok(src_fd) => {
            let src_stat = fstat(&src_fd)?;
            let src_info: FileDiskInfo = (src_stat.st_ino, src_stat.st_dev);
            let mut src_file = File::from(src_fd);
            io::copy(&mut src_file, &mut dst_file)?;
            Some(src_info)
        }
        Err(nix::errno::Errno::ENOENT) => {
            // no original file existing is fine
            None
        }
        Err(errno) => {
            return Err(errno.into());
        }
    };

    dst_file.sync_all()?;

    fchown(dst_file, owner_uid, None)?;

    Ok((src_info, dst_info))
}

fn copy_into_privileged(
    dst_dir: &OwnedFd,
    dst_filename: &OsStr,
    dst_info: Option<FileDiskInfo>,
    src_dir: &OwnedFd,
    src_filename: &OsStr,
    src_info: FileDiskInfo,
    editor_pid: i32,
) -> io::Result<()> {
    let (uid, gid, mode) = match fstatat(dst_dir, dst_filename, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(stat) => (stat.st_uid, stat.st_gid, stat.st_mode),
        Err(nix::errno::Errno::ENOENT) => (0, 0, 0o644),
        Err(errno) => {
            return Err(io::Error::from(errno));
        }
    };

    // TODO: xattrs
    // TODO: compare saved ino/dev of src/dst

    let tmp_fd = openat2(
        dst_dir,
        ".",
        OpenHow::new()
            .flags(OFlag::O_WRONLY | OFlag::O_CLOEXEC | OFlag::O_TMPFILE)
            .mode(Mode::empty())
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS | ResolveFlag::RESOLVE_BENEATH),
    )?;

    let src_fd = openat2(
        src_dir,
        src_filename,
        OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS | ResolveFlag::RESOLVE_BENEATH),
    )?;

    let src_stat = fstat(&src_fd)?;
    assert_eq!(src_stat.st_dev, src_info.1); // can't compare ino, because editor may attempt an atomic replace on this file

    let mut tmp_file = File::from(tmp_fd);
    let mut src_file = File::from(src_fd);
    io::copy(&mut src_file, &mut tmp_file)?;
    fchown(tmp_file.as_fd(), Some(uid), Some(gid))?;
    fchmod(tmp_file.as_fd(), Mode::from_bits_truncate(mode))?;
    tmp_file.sync_all()?;

    let mut tmp_name_linked: OsString = dst_filename.to_owned();
    tmp_name_linked.push(".tmp.");
    tmp_name_linked.push(editor_pid.to_string());

    linkat(
        tmp_file.as_fd(),
        "",
        dst_dir,
        tmp_name_linked.as_os_str(),
        AtFlags::AT_EMPTY_PATH,
    )?;

    drop(tmp_file);
    drop(src_file);

    match fstatat(dst_dir, dst_filename, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(dst_stat) => {
            assert!(dst_info.is_some());
            assert_eq!((dst_stat.st_ino, dst_stat.st_dev), dst_info.unwrap());
        }
        Err(nix::errno::Errno::ENOENT) => {
            assert!(dst_info.is_none());
        }
        Err(errno) => {
            return Err(io::Error::from(errno));
        }
    }

    renameat(dst_dir, tmp_name_linked.as_os_str(), dst_dir, dst_filename)?;
    fsync(dst_dir)?;

    unlinkat(
        src_dir,
        src_filename,
        nix::unistd::UnlinkatFlags::NoRemoveDir,
    )?;

    Ok(())
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let privileged_dir = open_dir(cli.file.parent().expect("file has parent"))?;
    let privileged_filename = cli.file.file_name().expect("file has name");
    let temp_dir = open_dir(cli.tmp_path.parent().expect("file has parent"))?;
    let temp_filename = cli.tmp_path.file_name().expect("file has name");

    let pidfd = pidfd_open(cli.editor_pid)?; // fail early if editor stopped existing

    let (privileged_info, temp_info) = copy_into_new_file(
        &privileged_dir,
        privileged_filename,
        &temp_dir,
        temp_filename,
        uid_from_pid(cli.editor_pid),
    )?;

    let mut fds = [PollFd::new(pidfd.as_fd(), PollFlags::POLLIN)];
    poll(&mut fds, PollTimeout::NONE)?;

    println!("editor terminated");

    copy_into_privileged(
        &privileged_dir,
        privileged_filename,
        privileged_info,
        &temp_dir,
        temp_filename,
        temp_info,
        cli.editor_pid,
    )?;

    Ok(())
}
