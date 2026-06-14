// SPDX-License-Identifier: BSD-3-Clause

use users::uid_t;

use crate::common::*;

mod args;
pub use args::Cli;

// https://github.com/trifectatechfoundation/sudo-rs/blob/09a5b9acdd462a1606e20f7c241d3b433fbf373a/src/defaults/mod.rs#L72-L78
// https://github.com/sudo-project/sudo/blob/d0a19ef42dd1377e6cbfa0076663406a9ab11920/plugins/sudoers/env.c#L133-L200
// WARNING: This is a Blocklist. Any random piece of software can look at some potentially dangerous environment variable.
// This list may desync with sudo over time. This is a parity issue.
// -E is a VERY bad idea design-wise. So bad, in fact, sudo-rs refuses to accept the flag entirely.
const ENV_DELETE_FIXED: &[&str] = &[
    "IFS",
    "CDPATH",
    "LOCALDOMAIN",
    "RES_OPTIONS",
    "HOSTALIASES",
    "NLSPATH",
    "PATH_LOCALE",
    "SHLIB_PATH",
    "LIBPATH",
    "AUTHSTATE",
    "KRB5_KTNAME",
    "VAR_ACE",
    "USR_ACE",
    "DLC_ACE",
    "TERMINFO",          /* terminfo, exclusive path to terminfo files */
    "TERMINFO_DIRS",     /* terminfo, path(s) to terminfo files */
    "TERMPATH",          /* termcap, path(s) to termcap files */
    "TERMCAP",           /* XXX - only if it starts with '/' */
    "ENV",               /* ksh, file to source before script runs */
    "BASH_ENV",          /* bash, file to source before script runs */
    "PS4",               /* bash, prefix for lines in xtrace mode */
    "GLOBIGNORE",        /* bash, globbing patterns to ignore */
    "BASHOPTS",          /* bash, initial "shopt -s" options */
    "SHELLOPTS",         /* bash, initial "set -o" options */
    "JAVA_TOOL_OPTIONS", /* java, extra command line options */
    "_JAVA_OPTIONS",     /* java, extra command line options (legacy) */
    "CLASSPATH",         /* java, class search path */
    "PERLIO_DEBUG",      /* perl, debugging output file */
    "PERLLIB",           /* perl, search path for modules/includes */
    "PERL5LIB",          /* perl 5, search path for modules/includes */
    "PERL5OPT",          /* perl 5, extra command line options */
    "PERL5DB",           /* perl 5, command used to load debugger */
    "FPATH",             /* ksh, search path for functions */
    "NULLCMD",           /* zsh, command for null file redirection */
    "READNULLCMD",       /* zsh, command for null file redirection */
    "ZDOTDIR",           /* zsh, search path for dot files */
    "TMPPREFIX",         /* zsh, prefix for temporary files */
    "PYTHONHOME",        /* python, module search path */
    "PYTHONPATH",        /* python, search path */
    "PYTHONINSPECT",     /* python, allow inspection */
    "PYTHONUSERBASE",    /* python, per user site-packages directory */
    "PYTHONSTARTUP",     /* python, interactive mode startup script */
    "RUBYLIB",           /* ruby, library load path */
    "RUBYOPT",           /* ruby, extra command line options */
    "NODE_OPTIONS",      /* node.js, extra command line options */
    "NODE_PATH",         /* node.js, module search path */
    "GIT_SSH_COMMAND",   /* git, custom SSH command */
    "GCONV_PATH",        /* glibc generic char set conversion iface */
    "USER",              /* skipped by --preserve-env without parameters */
];

const ENV_DELETE_PREFIX: &[&str] = &[
    "LD_",
    "_RLD",
    "LDR_",
    "KRB5_CONFIG",
    "GIT_CONFIG_", /* git, global configuration override */
    "_",           /* Underscore is a common marker for "internal" env vars*/
];

fn env_var_allowed(env_var: &str) -> bool {
    if ENV_DELETE_FIXED.contains(&env_var) {
        return false;
    }

    if ENV_DELETE_PREFIX
        .iter()
        .any(|deny| env_var.starts_with(deny))
    {
        return false;
    }

    if env_var.contains("=") {
        return false;
    }

    true
}

pub fn parse_to_run0_cli(
    cli: Cli,
    cwd: Option<String>,
    current_uid: uid_t,
    current_env: Vec<String>,
) -> Result<ShimResult, Error> {
    // Maybe migrate to `systemd-run --wait -P -q -G` ?
    if cli.edit {
        return Err(Error::Unsupported(String::from("--edit")));
    }

    if cli.list > 0 || cli.other_user.is_some() {
        return Err(Error::Unsupported(String::from("list mode")));
    }

    if cli.chroot.is_some() {
        return Err(Error::Unsupported(String::from("--chroot")));
    }

    if cli.remove_timestamp || cli.reset_timestamp {
        // potential solution: call RevokeTemporaryAuthorizations on org.freedesktop.PolicyKit1.Authority dbus
        return Err(Error::Unsupported(String::from(
            "removing or resetting authentication timestamps",
        )));
    }

    if cli.host.is_some() {
        // potential solution: raw systemd-run with `--host`
        return Err(Error::Unsupported(String::from("--host")));
    }

    if cli.preserve_groups {
        return Err(Error::Unsupported(String::from("--preserve-groups")));
    }

    if cli.background {
        // potential solution: raw systemd-run with `--no-block`
        return Err(Error::Unsupported(String::from("--background")));
    }

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
        buf.cli.push(String::from(POLKIT_STDIN_AGENT));
        buf.cli.push(String::from("--password-fd=0"));
        buf.cli.push(String::from("--"));
    }

    buf.cli.push(String::from(RUN0_CMD));

    if cli.shell || cli.login {
        buf.cli.push(String::from("--via-shell"));
    }

    if let Some(work_dir) = cli.working_directory.or(if cli.login {
        Some(String::from("~"))
    } else {
        cwd
    }) {
        buf.cli.push(format!("--chdir={work_dir}"));
    }

    if cli.non_interactive {
        buf.cli.push(String::from("--no-ask-password"))
    }

    if let Some(user) = cli.user {
        // FIXME: handle numerics safely
        buf.cli
            .push(format!("--user={}", user.trim_start_matches('#')))
    } else if cli.group.is_some() {
        buf.cli.push(format!("--user={}", current_uid))
    }

    if let Some(group) = cli.group {
        // FIXME: handle numerics safely
        buf.cli
            .push(format!("--group={}", group.trim_start_matches('#')))
    }

    let mut env_var_prefix_split_idx: usize = 0;
    for arg in cli.command.iter() {
        if arg.contains("=") && !arg.starts_with("=") && !arg.starts_with("/") {
            env_var_prefix_split_idx += 1;
        } else {
            break;
        }
    }

    let (extra_env_vars, command) = cli.command.split_at(env_var_prefix_split_idx);

    let env_var_flags = extra_env_vars
        .iter()
        .cloned()
        .chain(cli.preserve_env.map_or_else(Vec::new, |vars| {
            if vars.is_empty() {
                buf.push_stderr("run0-sudo-shim: Potentially insecure use of -E or --preserve-env without explicit list of preserved env vars");
                current_env
                    .into_iter()
                    .filter(|e| env_var_allowed(e))
                    .collect()
            } else {
                vars
            }
        }))
        .map(|e| format!("--setenv={e}"));

    buf.cli.extend(env_var_flags);

    if let Some(limit_nofile) = cli.file_descriptor_limit {
        buf.cli
            .push(format!("--property=LimitNOFILE={limit_nofile}"));
    }

    if let Some(timeout_secs) = cli.command_timeout {
        buf.cli
            .push(format!("--property=RuntimeMaxSec={timeout_secs}"));
    }

    buf.cli.extend(cli.run0_extra_args);
    buf.cli.push(String::from("--"));

    if cli.validate {
        buf.cli.push(String::from(TRUE_CMD));
    } else if !command.is_empty() {
        buf.cli.extend(command.to_vec());
    } else if !(cli.shell || cli.login) {
        return Err(Error::PrintHelp);
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn test_prog() {
        let cli = Cli::parse_from(["sudo", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--"),
                String::from("prog")
            ])
        );
    }
    #[test]
    fn test_bare() {
        let cli = Cli::parse_from(["sudo"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(build_result, Err(Error::PrintHelp));
    }

    #[test]
    fn test_chdir() {
        let cli = Cli::parse_from(["sudo", "-D", "/foo", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--chdir=/foo"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_stdin() {
        let cli = Cli::parse_from(["sudo", "--stdin", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(POLKIT_STDIN_AGENT),
                String::from("--password-fd=0"),
                String::from("--"),
                String::from(RUN0_CMD),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_close_from() {
        let cli = Cli::parse_from(["sudo", "-C", "1000", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--property=LimitNOFILE=1000"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_interactive() {
        let cli = Cli::parse_from(["sudo", "-i"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert!(build_result.is_ok());
        let args = build_result.unwrap().cli;
        assert!(args[0] == RUN0_CMD);
        assert!(args.contains(&String::from("--chdir=~")));
        assert!(args.contains(&String::from("--via-shell")));
    }

    #[test]
    fn test_preserve_env_selective() {
        let cli = Cli::parse_from(["sudo", "--preserve-env=foo,bar,baz", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--setenv=foo"),
                String::from("--setenv=bar"),
                String::from("--setenv=baz"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_preserve_env_global() {
        let cli = Cli::parse_from(["sudo", "-E", "prog"]);
        let build_result = parse_to_run0_cli(
            cli,
            None,
            1000,
            vec![
                String::from("foo"),
                String::from("bar"),
                String::from("baz"),
            ],
        );
        assert!(build_result.is_ok());
        let res = build_result.unwrap();
        assert_eq!(
            res.cli,
            vec![
                String::from(RUN0_CMD),
                String::from("--setenv=foo"),
                String::from("--setenv=bar"),
                String::from("--setenv=baz"),
                String::from("--"),
                String::from("prog")
            ]
        );
        assert!(res.get_stderr().contains("Potentially insecure use of -E"));
        assert!(res.get_stdout().is_empty());
    }

    #[test]
    fn test_preserve_env_global_strip() {
        let cli = Cli::parse_from(["sudo", "-E", "prog"]);
        let build_result = parse_to_run0_cli(
            cli,
            None,
            1000,
            vec![
                String::from("LD_LIBRARY_PATH"),
                String::from("LD_PRELOAD"),
                String::from("PYTHONPATH"),
            ],
        );
        assert!(build_result.is_ok());
        let res = build_result.unwrap();
        assert_eq!(
            res.cli,
            vec![
                String::from(RUN0_CMD),
                String::from("--"),
                String::from("prog")
            ]
        );
        assert!(res.get_stderr().contains("Potentially insecure use of -E"));
        assert!(res.get_stdout().is_empty());
    }

    #[test]
    fn test_set_env_prefix() {
        let cli = Cli::parse_from(["sudo", "foo=42", "bar=buzz", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--setenv=foo=42"),
                String::from("--setenv=bar=buzz"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_set_env_prefix_after_command() {
        // regression test for https://github.com/LordGrimmauld/run0-sudo-shim/issues/20
        let cli = Cli::parse_from(["sudo", "env", "-i", "foo=42", "ls"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--"),
                String::from("env"),
                String::from("-i"),
                String::from("foo=42"),
                String::from("ls"),
            ])
        );
    }

    #[test]
    fn test_set_env_prefix_skips_weird_1() {
        let cli = Cli::parse_from(["sudo", "foo=42", "=bar=buzz", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--setenv=foo=42"),
                String::from("--"),
                String::from("=bar=buzz"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_set_env_prefix_skips_weird_2() {
        let cli = Cli::parse_from(["sudo", "foo=42", "/bar=buzz", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--setenv=foo=42"),
                String::from("--"),
                String::from("/bar=buzz"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_group() {
        let cli = Cli::parse_from(["sudo", "-g", "dialout", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--user=1000"), // -g should maintain spawning user
                String::from("--group=dialout"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_group_and_user() {
        let cli = Cli::parse_from(["sudo", "-g", "dialout", "-u", "root", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--user=root"),
                String::from("--group=dialout"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_numeric_user() {
        let cli = Cli::parse_from(["sudo", "-u", "#0", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--user=0"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_named_user() {
        let cli = Cli::parse_from(["sudo", "-u", "root", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--user=root"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_non_interactive() {
        let cli = Cli::parse_from(["sudo", "-n", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--no-ask-password"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_shell_command() {
        let cli = Cli::parse_from(["sudo", "-s", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--via-shell"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_shell_bare() {
        let cli = Cli::parse_from(["sudo", "-s"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--via-shell"),
                String::from("--"),
            ])
        );
    }

    #[test]
    fn test_timeout() {
        let cli = Cli::parse_from(["sudo", "-T", "1000", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--property=RuntimeMaxSec=1000"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }

    #[test]
    fn test_validate() {
        let cli = Cli::parse_from(["sudo", "-v"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--"),
                String::from(TRUE_CMD)
            ])
        );
    }

    #[test]
    fn test_extra_arg() {
        let cli = Cli::parse_from(["sudo", "--run0-extra-arg=--background=42", "prog"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            ShimResult::ok_from(vec![
                String::from(RUN0_CMD),
                String::from("--background=42"),
                String::from("--"),
                String::from("prog")
            ])
        );
    }
}

#[cfg(test)]
mod unsupported {
    use clap::Parser;

    use super::*;

    #[test]
    fn test_background_unsupported() {
        let cli = Cli::parse_from(["sudo", "-b"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--background")))
        );
    }

    #[test]
    fn test_edit_unsupported() {
        let cli = Cli::parse_from(["sudo", "-e"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--edit")))
        );
    }

    #[test]
    fn test_host_unsupported() {
        let cli = Cli::parse_from(["sudo", "--host", "foo"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--host")))
        );
    }

    #[test]
    fn test_remove_timestamp_unsupported() {
        let cli = Cli::parse_from(["sudo", "-K"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from(
                "removing or resetting authentication timestamps"
            )))
        );
    }

    #[test]
    fn test_reset_timestamp_unsupported() {
        let cli = Cli::parse_from(["sudo", "-k"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from(
                "removing or resetting authentication timestamps"
            )))
        );
    }

    #[test]
    fn test_list_unsupported() {
        let cli = Cli::parse_from(["sudo", "-l"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("list mode")))
        );
    }

    #[test]
    fn test_preserve_group_unsupported() {
        let cli = Cli::parse_from(["sudo", "-P"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--preserve-groups")))
        );
    }

    #[test]
    fn test_chroot_unsupported() {
        let cli = Cli::parse_from(["sudo", "-R", "/"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("--chroot")))
        );
    }

    #[test]
    fn test_other_user_unsupported() {
        // FIXME: This should probably not even be legal CLI input
        let cli = Cli::parse_from(["sudo", "-U", "alice"]);
        let build_result = parse_to_run0_cli(cli, None, 1000, vec![]);
        assert_eq!(
            build_result,
            Err(Error::Unsupported(String::from("list mode")))
        );
    }
}
