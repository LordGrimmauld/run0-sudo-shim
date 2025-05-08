mod args;
use crate::args::Cli;
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    println!("Hello, world!");
}
