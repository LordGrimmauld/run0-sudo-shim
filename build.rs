use std::{
    fs, io,
    path::{Path, PathBuf},
};

// include *only* clap argument parsing, not runtime code
// needed for man generation
#[path = "src/run0-sudo-shim/args.rs"]
mod args;
#[path = "src/run0-sudo-shim/sudo/args.rs"]
mod sudo;

#[cfg(feature = "run0-edit-daemon")]
#[path = "src/run0-edit-daemon/args.rs"]
mod run0_edit_daemon;

use clap::{Command, CommandFactory};

use clap_complete::{generate_to, shells::Shell};

use crate::args::Cli;

fn gen_for_command(
    mut cmd: Command,
    manpage_out_dir: &Path,
    completion_out_dir: &Path,
) -> io::Result<()> {
    let name = cmd.get_name().to_owned();
    let section = "8";

    println!("Generating docs for command: {name}");

    generate_to(Shell::Bash, &mut cmd, &name, completion_out_dir)?;
    generate_to(Shell::Zsh, &mut cmd, &name, completion_out_dir)?;
    generate_to(Shell::Fish, &mut cmd, &name, completion_out_dir)?;

    let man = clap_mangen::Man::new(cmd).section(section);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    let filename = format!("{name}.{section}");
    fs::write(manpage_out_dir.join(filename), buffer)?;

    Ok(())
}

// inspired and adapted from bottom man page generation: https://github.com/ClementTsang/bottom/blob/d3c2223e5122079b04e72baf86f21397b35620ec/build.rs#L39-L77
fn main() -> io::Result<()> {
    let completion_dir =
        option_env!("COMPLETION_DIR").unwrap_or("./target/tmp/run0-sudo-shim/completion/");
    let manpage_dir = option_env!("MANPAGE_DIR").unwrap_or("./target/tmp/run0-sudo-shim/manpage/");
    let manpage_out_dir = PathBuf::from(manpage_dir);
    let completion_out_dir = PathBuf::from(completion_dir);
    fs::create_dir_all(&manpage_out_dir)?;
    fs::create_dir_all(&completion_out_dir)?;

    for sub in Cli::command().get_subcommands() {
        gen_for_command(sub.clone(), &manpage_out_dir, &completion_out_dir)?;
    }

    #[cfg(feature = "run0-edit-daemon")]
    {
        let cmd = crate::run0_edit_daemon::Cli::command();
        gen_for_command(cmd, &manpage_out_dir, &completion_out_dir)?;
    }

    Ok(())
}
