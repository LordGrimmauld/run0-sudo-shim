use std::{fs, io, path::PathBuf};

// include *only* clap argument parsing, not runtime code
// needed for man generation
#[path = "src/run0-sudo-shim/args.rs"]
mod args;
#[path = "src/run0-sudo-shim/sudo/args.rs"]
mod sudo;

use clap::CommandFactory;

use crate::args::Cli;

static COMMANDS: [(&str, &str); 1] = [("sudo", "8")];

// inspired and adapted from bottom man page generation: https://github.com/ClementTsang/bottom/blob/d3c2223e5122079b04e72baf86f21397b35620ec/build.rs#L39-L77
fn main() -> io::Result<()> {
    let manpage_dir = option_env!("MANPAGE_DIR").unwrap_or("./target/tmp/run0-sudo-shim/manpage/");
    let manpage_out_dir = PathBuf::from(manpage_dir);
    fs::create_dir_all(&manpage_out_dir)?;

    let mut root = Cli::command();

    for &(name, section) in &COMMANDS {
        let filename = format!("{name}.{section}");
        println!("{name} -> {filename}");
        if let Some(sub) = root.find_subcommand_mut(name) {
            let man = clap_mangen::Man::new(sub.clone()).section(section);
            let mut buffer: Vec<u8> = Default::default();
            man.render(&mut buffer)?;
            fs::write(manpage_out_dir.join(filename), buffer)?;
        }
    }

    Ok(())
}
