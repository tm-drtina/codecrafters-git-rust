use std::fs::{self, File};
use std::io::{prelude::*, stdout};

use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;

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
            fs::create_dir(".git").context("Create root dir")?;
            fs::create_dir(".git/objects").context("Create objects dir")?;
            fs::create_dir(".git/refs").context("Create refs dir")?;
            fs::write(".git/HEAD", "ref: refs/heads/master\n").context("Write HEAD")?;
            println!("Initialized git directory")
        }
        Commands::CatFile {
            pretty_print,
            object,
        } => {
            ensure!(pretty_print, "Only pretty-print is supported!");
            let (prefix, filename) = object.split_at(2);
            let file = File::open(format!(".git/objects/{}/{}", prefix, filename))
                .context("Opening object file")?;
            let mut decoder = ZlibDecoder::new(file);
            let mut buf = Vec::new();
            decoder
                .read_to_end(&mut buf)
                .context("Reading object file")?;
            stdout().lock().write_all(&buf).context("Writing result")?;
        }
    }
    Ok(())
}
