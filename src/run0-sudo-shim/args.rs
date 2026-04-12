// SPDX-License-Identifier: GPL-3.0-only

use std::process::exit;

#[derive(Debug, Default)]
pub struct Cli {
    pub bell: bool,
    pub file_descriptor_limit: Option<u32>,
    pub working_directory: Option<String>,
    /// None = not given, Some([]) = -E / --preserve-env (all), Some(list) = --preserve-env=A,B
    pub preserve_env: Option<Vec<String>>,
    pub edit: bool,
    pub group: Option<String>,
    pub set_home: bool,
    pub login: bool,
    pub list: u8,
    pub non_interactive: bool,
    pub chroot: Option<String>,
    pub stdin: bool,
    pub other_user: Option<String>,
    pub user: Option<String>,
    pub validate: bool,
    pub run0_extra_args: Vec<String>,
    pub command: Vec<String>,
}

/// (short, long, takes_value). '\0' = no short form.
/// Options whose value we ignore are still listed so they parse correctly.
const OPTS: &[(char, &str, bool)] = &[
    ('A', "askpass", false),
    ('b', "background", false),
    ('B', "bell", false),
    ('C', "close-from", true),
    ('D', "chdir", true),
    ('E', "preserve-env", false), // value only via --preserve-env=LIST
    ('e', "edit", false),
    ('g', "group", true),
    ('H', "set-home", false),
    ('\0', "host", false),
    ('i', "login", false),
    ('K', "remove-timestamp", false),
    ('k', "reset-timestamp", false),
    ('l', "list", false),
    ('n', "non-interactive", false),
    ('P', "preserve-groups", false),
    ('p', "prompt", true),
    ('R', "chroot", true),
    ('S', "stdin", false),
    ('s', "shell", false),
    ('T', "command-timeout", true),
    ('U', "other-user", true),
    ('u', "user", true),
    ('v', "validate", false),
    ('\0', "run0-extra-arg", true),
    ('h', "help", false),
    ('V', "version", false),
];

fn lookup_short(c: char) -> Option<(&'static str, bool)> {
    OPTS.iter()
        .find(|(s, ..)| *s == c)
        .map(|(_, l, v)| (*l, *v))
}

fn lookup_long(name: &str) -> Option<bool> {
    OPTS.iter().find(|(_, l, _)| *l == name).map(|(.., v)| *v)
}

impl Cli {
    pub fn parse() -> Self {
        Self::parse_from(std::env::args().skip(1))
    }

    pub fn parse_from<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut cli = Cli::default();
        let mut args = iter.into_iter().map(Into::into);

        while let Some(arg) = args.next() {
            if arg == "--" {
                cli.command.extend(args);
                break;
            }

            if let Some(long) = arg.strip_prefix("--") {
                let (name, inline) = match long.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (long, None),
                };
                if let Some(needs_val) = lookup_long(name) {
                    let val =
                        if needs_val {
                            Some(inline.or_else(|| args.next()).unwrap_or_else(|| {
                                die(&format!("option --{name} requires a value"))
                            }))
                        } else {
                            inline
                        };
                    cli.apply(name, val);
                    continue;
                }
            } else if let Some(shorts) = arg.strip_prefix('-')
                && !shorts.is_empty()
                && cli.apply_short_cluster(shorts, &mut args)
            {
                continue;
            }

            // Positional or unknown option: this and everything after is the command.
            cli.command.push(arg);
            cli.command.extend(args);
            break;
        }
        cli
    }

    /// Returns false if an unknown short was hit (caller treats whole arg as command).
    fn apply_short_cluster(
        &mut self,
        cluster: &str,
        rest: &mut impl Iterator<Item = String>,
    ) -> bool {
        for (i, c) in cluster.char_indices() {
            let Some((name, needs_val)) = lookup_short(c) else {
                return false;
            };
            if needs_val {
                // -uroot or -u root
                let tail = &cluster[i + c.len_utf8()..];
                let val = if tail.is_empty() {
                    rest.next()
                        .unwrap_or_else(|| die(&format!("option -{c} requires a value")))
                } else {
                    tail.to_string()
                };
                self.apply(name, Some(val));
                return true;
            }
            self.apply(name, None);
        }
        true
    }

    /// `name` is always a known long name (lookup happens in the caller).
    fn apply(&mut self, name: &str, val: Option<String>) {
        match name {
            "help" => {
                print_help();
                exit(0)
            }
            "version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                exit(0)
            }
            "bell" => self.bell = true,
            "close-from" => {
                let v = val.unwrap();
                self.file_descriptor_limit = Some(
                    v.parse()
                        .unwrap_or_else(|_| die(&format!("--close-from: invalid number '{v}'"))),
                )
            }
            "chdir" => self.working_directory = val,
            "preserve-env" => {
                let list = self.preserve_env.get_or_insert_with(Vec::new);
                if let Some(v) = val {
                    list.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string));
                }
            }
            "edit" => self.edit = true,
            "group" => self.group = val,
            "set-home" => self.set_home = true,
            "login" => self.login = true,
            "list" => self.list = self.list.saturating_add(1),
            "non-interactive" => self.non_interactive = true,
            "chroot" => self.chroot = val,
            "stdin" => self.stdin = true,
            "other-user" => self.other_user = val,
            "user" => self.user = val,
            "validate" => self.validate = true,
            "run0-extra-arg" => self.run0_extra_args.push(val.unwrap()),
            // accepted-but-ignored sudo options
            "askpass" | "background" | "host" | "remove-timestamp" | "reset-timestamp"
            | "preserve-groups" | "prompt" | "shell" | "command-timeout" => {}
            _ => unreachable!("{name}"),
        }
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{}: {msg}", env!("CARGO_PKG_NAME"));
    exit(2)
}

pub fn print_help() {
    println!(
        "{name} {ver}
{desc}

USAGE:
    {name} [OPTIONS] [--] [COMMAND]...

OPTIONS:
    -A, --askpass                 [IGNORED] use a helper program for password prompting
    -b, --background              [IGNORED] run command in the background
    -B, --bell                    ring bell when prompting
    -C, --close-from <N>          set NOFILE limit (approximates sudo's fd closing)
    -D, --chdir <DIR>             change the working directory before running command
    -E, --preserve-env[=VARS]     preserve user environment when running command
    -e, --edit                    [UNSUPPORTED] edit files instead of running a command
    -g, --group <GROUP>           run command as the specified group name or ID
    -H, --set-home                set HOME variable to target user's home dir
        --host                    [IGNORED] run command on host
    -i, --login                   run login shell as the target user
    -K, --remove-timestamp        [IGNORED] remove timestamp file completely
    -k, --reset-timestamp         [IGNORED] invalidate timestamp file
    -l, --list                    [UNSUPPORTED] list user's privileges
    -n, --non-interactive         non-interactive mode, no prompts are used
    -P, --preserve-groups         [IGNORED] preserve group vector
    -p, --prompt <PROMPT>         [IGNORED] use the specified password prompt
    -R, --chroot <DIR>            [UNSUPPORTED] change the root directory
    -S, --stdin                   [UNSUPPORTED] read password from standard input
    -s, --shell                   [IGNORED] run shell as the target user
    -T, --command-timeout <T>     [IGNORED] terminate command after the specified time limit
    -U, --other-user <USER>       [UNSUPPORTED] in list mode, display privileges for user
    -u, --user <USER>             run command as specified user name or ID
    -v, --validate                validate a root login
        --run0-extra-arg <ARG>    extra argument to pass to run0 (repeatable)
    -h, --help                    print help
    -V, --version                 print version",
        name = env!("CARGO_PKG_NAME"),
        ver = env!("CARGO_PKG_VERSION"),
        desc = env!("CARGO_PKG_DESCRIPTION"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Cli {
        Cli::parse_from(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn user_and_command() {
        let c = p(&["-u", "root", "id", "-a"]);
        assert_eq!(c.user.as_deref(), Some("root"));
        assert_eq!(c.command, vec!["id", "-a"]);
    }

    #[test]
    fn clustered_short_with_value() {
        let c = p(&["-uroot", "true"]);
        assert_eq!(c.user.as_deref(), Some("root"));
        assert_eq!(c.command, vec!["true"]);
    }

    #[test]
    fn clustered_flags() {
        let c = p(&["-nki", "true"]);
        assert!(c.non_interactive && c.login);
        assert_eq!(c.command, vec!["true"]);
    }

    #[test]
    fn preserve_env_variants() {
        let c = p(&["-E", "true"]);
        assert_eq!(c.preserve_env, Some(vec![]));
        assert_eq!(c.command, vec!["true"]);

        let c = p(&["--preserve-env=PATH,HOME", "true"]);
        assert_eq!(
            c.preserve_env,
            Some(vec!["PATH".to_string(), "HOME".to_string()])
        );
    }

    #[test]
    fn double_dash() {
        let c = p(&["--", "-u", "root"]);
        assert_eq!(c.command, vec!["-u", "root"]);
        assert_eq!(c.user, None);
    }

    #[test]
    fn list_count() {
        let c = p(&["-ll"]);
        assert_eq!(c.list, 2);
    }

    #[test]
    fn run0_extra_arg_hyphen_value() {
        let c = p(&[
            "--run0-extra-arg=--pty",
            "--run0-extra-arg",
            "--nice=5",
            "id",
        ]);
        assert_eq!(c.run0_extra_args, vec!["--pty", "--nice=5"]);
        assert_eq!(c.command, vec!["id"]);
    }

    #[test]
    fn unknown_option_starts_command() {
        let c = p(&["-n", "./script", "--weird-flag"]);
        assert!(c.non_interactive);
        assert_eq!(c.command, vec!["./script", "--weird-flag"]);
    }

    #[test]
    fn ignored_value_option_consumes_arg() {
        // -p takes a value we ignore; must not eat the command.
        let c = p(&["-p", "Password:", "id"]);
        assert_eq!(c.command, vec!["id"]);
    }
}
