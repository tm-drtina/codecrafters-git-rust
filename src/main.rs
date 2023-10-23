use std::fs::File;

use anyhow::{ensure, Result, Context};
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
    HashObject {
        #[arg(short)]
        write: bool,
        file: String,
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
            let obj = Object::read(object)?;
            obj.print_pretty()?;
        }
        Commands::HashObject { write, file } => {
            let file = File::open(file).context("Open input file")?;
            let obj = Object::create(file)?;
            if write {
                obj.write()?;
            }
            println!("{}", obj.hash);
        },
    }
    Ok(())
}
