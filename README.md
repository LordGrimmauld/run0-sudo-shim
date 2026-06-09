# Why

`run0-sudo-shim` attempts to imitate sudo as close as possible, while actually using `run0` in the back.

`run0` does not rely on SUID binaries, which makes it a more secure option.
It is also already included in any systemd-based linux installation.

However, many programs just expect sudo to exist, so a shim is necessary to make those legacy programs work.
`run0-sudo-shim` is meant for a migration period only. Eventually, all users should migrate to `run0`,
`systemd-run`, or other socket-activated elevator tools. Even more preferable to that would be bespoke socket-activated
services with a well-defined API, but that migration will take more time yet.

# Security

`run0-sudo-shim` is an unprivileged non-SUID binary rewriting `sudo` cli invocation into a `run0` invocation.
`run0-sudo-shim` does not enforce security boundaries. It serves to only build an invocation of
`run0`, which then has to enforces security boundaries.
Anything a user can pass to the shim, the user can pass directly to `run0`.

Security issues are one of:
1. the NixOS module provided by the flake.nix (silently) modifies a users global system to be less secure
2. the shim silently fails to perform a security action the caller requested (credential drop, env scrubbing, confinement to a non‑root user)
3. a tool written against real `sudo` semantics passes *untrusted* data through the shim and gets a more privileged result than real `sudo` would have produced

Differences in behavior between this shim and `sudo` are considered bugs, but not considered security issues.

# Unsupported Options

This shim will never read `/etc/sudoers`. The shim is unprivileged:
It does not have read privileges on `/etc/sudoers`,
and can not effectively enforce security against the user executing the shim.

Security features of sudo that are unsupported (such as `--remove-timestamp`/`--reset-timestamp`)
will exit and emit an error on `stderr`, as to not suggest security actions have succeeded despite not being run at all.

`sudoedit`/`sudo -e` is currently not supported. Supporting this safely is quite complex, and may happen in a future version of this shim.

`sudo -E` (preserving environment without an explicit list) strips some potentially dangerous environment variables.
This is not a security boundary: deny-lists like this are inherently incomplete. This is only a measure against footguns.
`sudo --preserve-env=<...> ...`/`sudo FOO=bar ...` does NOT make an attempt at stripping environment variables.
This is equivalent to `SETENV: ALL` in `/etc/sudoers`. Security implications of this are enforced by systemd `run0`.

# Supported Options

```
Shim for the sudo command that utilizes run0

Usage: sudo [OPTIONS] [COMMAND]...

Arguments:
  [COMMAND]...  command to be executed

Options:
  -A, --askpass
          [IGNORED] use a helper program for password prompting
  -b, --background
          [UNSUPPORTED] run command in the background
  -B, --bell
          ring bell when prompting
  -C, --close-from <FILE_DESCRIPTOR_LIMIT>
          diverging from sudo, this sets NOFILE limit, achieving similar behavior as sudo explicitly watching and killing file descriptors
  -D, --chdir <WORKING_DIRECTORY>
          change the working directory before running command
  -E, --preserve-env[=<PRESERVE_ENV>...]
          preserve user environment when running command. If no explicit list of environment variables is supplied, preserves all variables except a narrow blocklist. This is considered insecure and a warning will be emitted
  -e, --edit
          [UNSUPPORTED] edit files instead of running a command
  -g, --group <GROUP>
          run command as the specified group name or ID
  -H, --set-home
          set HOME variable to target user's home dir
      --host <HOST>
          [UNSUPPORTED] run command on host (if supported by plugin)
  -i, --login
          run login shell as the target user; a command may also be specified
  -K, --remove-timestamp
          [UNSUPPORTED] remove timestamp file completely
  -k, --reset-timestamp
          [UNSUPPORTED] invalidate timestamp file
  -l, --list...
          [UNSUPPORTED] list user's privileges or check a specific command; use twice for longer format
  -n, --non-interactive
          non-interactive mode, no prompts are used
  -P, --preserve-groups
          [UNSUPPORTED] preserve group vector instead of setting to target's
  -p, --prompt <PROMPT>
          [IGNORED] use the specified password prompt
  -R, --chroot <CHROOT>
          [UNSUPPORTED] change the root directory before running command
  -S, --stdin
          read password from standard input
  -s, --shell
          run shell as the target user; a command may also be specified
  -T, --command-timeout <COMMAND_TIMEOUT>
          terminate command after the specified time limit
  -U, --other-user <OTHER_USER>
          [UNSUPPORTED] in list mode, display privileges for user
  -u, --user <USER>
          run command (or edit file) as specified user name or ID
  -v, --validate
          validate a root login
      --run0-extra-arg <RUN0_EXTRA_ARGS>
          an extra argument to pass to run0 (can be specified multiple times)
  -h, --help
          Print help
  -V, --version
          Print version
```

# Installing

`run0-sudo-shim` is a simple rust binary, which can be built with `cargo`. It does not require SUID binaries, nor does it require its own polkit rules.

## Installation as a Flake

Put in your inputs:

```nix
run0-sudo-shim = {
  url = "github:lordgrimmauld/run0-sudo-shim";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Put in your modules:

```nix
inputs.run0-sudo-shim.nixosModules.default
```

Put in your environment.systemPackages:

```nix
environment.systemPackages = [ pkgs.run0-sudo-shim ]
```
