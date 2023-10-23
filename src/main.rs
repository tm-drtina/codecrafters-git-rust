use std::fs::File;
use std::path::PathBuf;

use anyhow::{ensure, Context, Result};
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
        file: PathBuf,
    },
    LsTree {
        #[arg(long)]
        name_only: bool,
        tree_sha: String,
    },
    WriteTree,
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
            let obj = object::Object::read(object)?;
            obj.print_pretty()?;
        }
        Commands::HashObject { write, file } => {
            let file = File::open(file).context("Open input file")?;
            let obj = object::Object::create_blob(file)?;
            if write {
                obj.write()?;
            }
            println!("{}", obj.hash);
        }
        Commands::LsTree {
            name_only,
            tree_sha,
        } => {
            ensure!(name_only, "Only name-only mode is supported!");
            let obj = object::Object::read(tree_sha)?;
            let t = tree::Tree::try_from(obj)?;
            for entry in t.entries {
                println!("{}", entry.name);
            }
        }
        Commands::WriteTree => {
            let obj = tree::Tree::create(&std::env::current_dir()?)?.into_object();
            obj.write()?;
            println!("{}", obj.hash);
        }
    }
    Ok(())
}
