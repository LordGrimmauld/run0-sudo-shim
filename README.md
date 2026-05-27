`run0-sudo-shim` attempts to imitate sudo as close as possible, while actually using `run0` in the back.

`run0` does not rely on SUID binaries, which makes it a more secure option.
It is also included in any systemd-based linux installation.

However, many programs just expect sudo to exist, so a shim is necessary to make those programs work.

```
Shim for the sudo command that utilizes run0

Usage: sudo [OPTIONS] [COMMAND]...

Arguments:
  [COMMAND]...  command to be executed

Options:
  -A, --askpass
          [IGNORED] use a helper program for password prompting
  -b, --background
          [IGNORED] run command in the background
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
          [IGNORED] preserve group vector instead of setting to target's
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

#### Installation as a Flake

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
