mod args;

use crate::args::Cli;
use clap::Parser;

#[cfg(feature = "audit")]
use linux_audit_parser::MessageType;
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
#[cfg(feature = "audit")]
use std::{
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    thread,
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

#[cfg(feature = "audit")]
fn setup_audit_rule(editor_pid: i32, temp_path: &Path) -> io::Result<()> {
    use common::external_programs::AUDITCTL_CMD;
    use std::process::Command;
    let res = Command::new(AUDITCTL_CMD)
        .arg("-w")
        .arg(temp_path.as_os_str())
        .args(["-p", "wa"])
        .args(["-k", &format!("run0-edit-{editor_pid}")])
        .status();
    match res {
        Ok(status) => {
            if !status.success() {
                eprintln!(
                    "Error while cleaning up audit logging: `auditctl` failed with status: {}",
                    status
                );
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    status.to_string(),
                ))
            } else {
                Ok(())
            }
        }
        Err(err) => {
            eprintln!("Error while setting up audit logging: {err}");
            Err(err)
        }
    }
}

#[cfg(feature = "audit")]
fn teardown_audit_rule(editor_pid: i32, temp_path: &Path) {
    use common::external_programs::AUDITCTL_CMD;
    use std::process::Command;
    let res = Command::new(AUDITCTL_CMD)
        .arg("-W")
        .arg(temp_path.as_os_str())
        .args(["-p", "wa"])
        .args(["-k", &format!("run0-edit-{editor_pid}")])
        .status();
    match res {
        Ok(status) => {
            if !status.success() {
                eprintln!(
                    "Error while cleaning up audit logging: `auditctl` failed with status: {}",
                    status
                );
            }
        }
        Err(err) => eprintln!("Error while cleaning up audit logging: {err}"),
    };
}

#[cfg(feature = "audit")]
pub fn start_audit_logger(editor_pid: i32) -> thread::JoinHandle<()> {
    let wanted_key = format!("run0-edit-{editor_pid}");

    thread::spawn(move || {
        let stream = match UnixStream::connect(common::AUDISP_SOCKET) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("Failed to connect to {}: {err}", common::AUDISP_SOCKET);
                return;
            }
        };

        let mut reader = BufReader::new(stream);

        let mut buf = Vec::new();
        loop {
            buf.clear();
            // need to actually keep \n around for parser to be happy
            if reader.read_until(b'\n', &mut buf).is_err() || buf.is_empty() {
                break;
            }

            let parsed = linux_audit_parser::parse(&buf, true);
            match parsed {
                Ok(res) => {
                    if let Some(linux_audit_parser::Value::Str(key, _)) = res.body.get("key")
                        && key == &wanted_key.as_bytes()
                        && res.ty == MessageType::SYSCALL
                    {
                        match res.body.get("pid") {
                            Some(linux_audit_parser::Value::Number(
                                linux_audit_parser::Number::Dec(pid),
                            )) => {
                                if pid == &(editor_pid as i64) {
                                    println!("detected editor access from pid {pid}");
                                } else if pid == &std::process::id().into() {
                                    println!("detected daemon access from pid {pid}");
                                } else {
                                    eprintln!(
                                        "detected anomalous access from pid {pid}: {:?}",
                                        res
                                    );
                                    // TODO: actually *do something* about some other process messing with the temp file
                                }
                            }
                            _ => eprintln!(
                                "audit detected access, but unable to read pid: {:?}",
                                res
                            ),
                        }
                    }
                }
                Err(e) => eprintln!("audit parsing failed: {e}"),
            }
        }
    })
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let privileged_dir = open_dir(cli.file.parent().expect("file has parent"))?;
    let privileged_filename = cli.file.file_name().expect("file has name");
    let temp_dir = open_dir(cli.tmp_path.parent().expect("file has parent"))?;
    let temp_filename = cli.tmp_path.file_name().expect("file has name");

    let pidfd = pidfd_open(cli.editor_pid)?; // fail early if editor stopped existing

    #[cfg(feature = "audit")]
    let audit_needs_teardown: bool = {
        start_audit_logger(cli.editor_pid);
        setup_audit_rule(cli.editor_pid, &cli.tmp_path).is_ok()
    };

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

    #[cfg(feature = "audit")]
    if audit_needs_teardown {
        teardown_audit_rule(cli.editor_pid, &cli.tmp_path);
    }

    Ok(())
}

#[cfg(all(feature = "audit", test))]
mod audit_parser_smoketest {
    #[test]
    fn test_audit_parser_can_parse() {
        // just a simple smoke test of the kind of entries we get
        let line = "type=SYSCALL msg=audit(1786032480.133:844): arch=c000003e syscall=437 success=yes exit=7 a0=4 a1=7ffe98d6e2b0 a2=7ffe98d6e820 a3=18 items=2 ppid=1 pid=9333 auid=4294967295 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=(none) ses=4294967295 comm=\"run0-edit-daemo\" exe=\"/nix/store/srj668z1f7hw43ffl0gpc5wfbcg0l7hs-run0-sudo-shim/bin/run0-edit-daemon\" subj=unconfined key=\"run0-edit-9304\"\x1dARCH=x86_64 SYSCALL=openat2 AUID=\"unset\" UID=\"root\" GID=\"root\" EUID=\"root\" SUID=\"root\" FSUID=\"root\" EGID=\"root\" SGID=\"root\" FSGID=\"root\"\x0a";
        let parsed = linux_audit_parser::parse(line.as_bytes(), true);
        println!("{:?}", parsed);
        assert!(parsed.is_ok());
    }
}
