use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use git_starter_rust::*;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    CatFile {
        #[arg(short = 'p')]
        pretty_print: bool,
        object: String,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Init => {
            init()?;
            println!("Initialized git directory")
        }
        Commands::CatFile {
            pretty_print,
            object,
        } => {
            ensure!(pretty_print, "Only pretty-print is supported!");
            let obj = Object::read(&object)?;
            obj.print_pretty()?;
        }
    }
    Ok(())
}
