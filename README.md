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
          [IGNORED] close all file descriptors >= num
  -D, --chdir <WORKING_DIRECTORY>
          change the working directory before running command
  -E, --preserve-env[=<PRESERVE_ENV>...]
          preserve user environment when running command
  -e, --edit
          [UNSUPPORTED] edit files instead of running a command
  -g, --group <GROUP>
          run command as the specified group name or ID
  -H, --set-home
          set HOME variable to target user's home dir
      --host
          [IGNORED] run command on host (if supported by plugin)
  -i, --login
          run login shell as the target user; a command may also be specified
  -K, --remove-timestamp
          [IGNORED] remove timestamp file completely
  -k, --reset-timestamp
          [IGNORED] invalidate timestamp file
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
          [UNSUPPORTED] read password from standard input
  -s, --shell
          [IGNORED] run shell as the target user; a command may also be specified
  -T, --command-timeout <COMMAND_TIMEOUT>
          [IGNORED] terminate command after the specified time limit
  -U, --other-user <OTHER_USER>
          [UNSUPPORTED] in list mode, display privileges for user
  -u, --user <USER>
          run command (or edit file) as specified user name or ID
  -v, --validate
          validate a root login
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

```
inputs.run0-sudo-shim.nixosModules.default
```

Put in your environment.systemPackages:

```
pkgs.run0-sudo-shim
```
