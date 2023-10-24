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
    CommitTree {
        tree_sha: String,
        #[arg(short)]
        parent: Option<String>,
        #[arg(short)]
        message: String,
    }
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
            let obj: object::Object = File::open(file).context("Open input file")?.try_into()?;
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
            let obj = tree::Tree::write(&std::env::current_dir()?)?;
            println!("{}", obj.hash);
        }
        Commands::CommitTree { tree_sha, parent, message } => {
            let author = commit::Author {
                name: String::from("Bob"),
                email: String::from("bob@example.com"),
                time: std::time::SystemTime::now(),
                time_offset: String::from("+0200"),
            };
            let c = commit::Commit {
                tree_sha,
                parent,
                author: author.clone(),
                commiter: author,
                message,
            };
            let obj: object::Object = c.try_into()?;
            obj.write()?;
            println!("{}", obj.hash);
        },
    }
    Ok(())
}
