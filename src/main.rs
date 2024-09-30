use std::fs::File;
use std::path::PathBuf;

use anyhow::{bail, ensure, Context, Result};
use clap::{Parser, Subcommand};
use codecrafters_git::commit::{Author, Commit};
use codecrafters_git::http_protocol::GitHttpClient;
use codecrafters_git::object::Object;
use codecrafters_git::tree::Tree;
use codecrafters_git::GitRepo;

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
    },
    Clone {
        repo_url: String,
        dest: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Init => {
            GitRepo::new_in_cwd()?.init()?;
            println!("Initialized git directory")
        }
        Commands::CatFile {
            pretty_print,
            object,
        } => {
            ensure!(pretty_print, "Only pretty-print is supported!");
            let repo = GitRepo::new_in_cwd()?;
            let obj = Object::read(&repo, object)?;
            obj.print_pretty()?;
        }
        Commands::HashObject { write, file } => {
            let repo = GitRepo::new_in_cwd()?;
            let obj: Object = File::open(file).context("Open input file")?.try_into()?;
            if write {
                obj.write(&repo)?;
            }
            println!("{}", obj.hash);
        }
        Commands::LsTree {
            name_only,
            tree_sha,
        } => {
            ensure!(name_only, "Only name-only mode is supported!");
            let repo = GitRepo::new_in_cwd()?;
            let obj = Object::read(&repo, tree_sha)?;
            let t = Tree::try_from(obj)?;
            for entry in t.entries {
                println!("{}", entry.name);
            }
        }
        Commands::WriteTree => {
            let repo = GitRepo::new_in_cwd()?;
            let obj = Tree::write(&repo, &repo.repo_root)?;
            println!("{}", obj.hash);
        }
        Commands::CommitTree {
            tree_sha,
            parent,
            message,
        } => {
            let repo = GitRepo::new_in_cwd()?;
            let author = Author {
                name: String::from("Bob"),
                email: String::from("bob@example.com"),
                time: std::time::SystemTime::now(),
                time_offset: String::from("+0200"),
            };
            let c = Commit {
                tree_sha,
                parent,
                author: author.clone(),
                commiter: author,
                message,
            };
            let obj: Object = c.try_into()?;
            obj.write(&repo)?;
            println!("{}", obj.hash);
        }
        Commands::Clone { repo_url, dest } => {
            std::fs::create_dir_all(&dest)?;
            let repo = GitRepo::new(&dest);
            repo.init()?;
            let http_client = GitHttpClient::new(&repo, repo_url);
            let ref_info = http_client.ref_info()?;
            http_client.fetch_refs(ref_info.refs.iter().map(|r| &r.id).collect())?;

            if let Some(r) = ref_info.refs.first() {
                if r.name == "HEAD" {
                    repo.checkout(String::from_utf8(r.id.to_vec())?)?;
                }
            } else {
                bail!("Missing HEAD reference");
            }
        }
    }
    Ok(())
}
