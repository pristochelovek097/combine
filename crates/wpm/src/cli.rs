use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug)]
pub enum Command {
    Add(Vec<String>),
    Remove(Vec<String>),
    List,
    ListInstalled,
    Search(String),
    Info(String),
    Update,
    Verify,
    Clean,
    Help,
    Version,
}

#[derive(Debug)]
pub struct Cli {
    pub config: Config,
    pub command: Command,
}

pub fn parse(args: Vec<String>) -> Result<Cli, String> {
    let mut config = Config::default();
    let mut positional = Vec::new();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--root requires a directory".to_string())?;
                config.root = PathBuf::from(value);
            }
            "--repo" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--repo requires a URL".to_string())?;
                config.repo = value;
            }
            "--no-verify" => config.verify = false,
            "--verify" => config.verify = true,
            "--musl" => config.musl = true,
            "--reinstall" => config.reinstall = true,
            "-h" | "--help" => {
                return Ok(Cli {
                    config,
                    command: Command::Help,
                })
            }
            "-V" | "--version" => {
                return Ok(Cli {
                    config,
                    command: Command::Version,
                })
            }
            _ => positional.push(arg),
        }
    }

    let command = match positional.first().map(String::as_str) {
        None => Command::Help,
        Some("add") | Some("install") => {
            let names = positional[1..].to_vec();
            if names.is_empty() {
                return Err("add requires at least one package name".to_string());
            }
            Command::Add(names)
        }
        Some("remove") | Some("rm") | Some("del") => {
            let names = positional[1..].to_vec();
            if names.is_empty() {
                return Err("remove requires at least one package name".to_string());
            }
            Command::Remove(names)
        }
        Some("list") => {
            if positional.get(1).map(String::as_str) == Some("-i") {
                Command::ListInstalled
            } else {
                Command::List
            }
        }
        Some("search") => {
            let query = positional.get(1).ok_or("search requires a query")?.clone();
            Command::Search(query)
        }
        Some("info") | Some("show") => {
            let name = positional
                .get(1)
                .ok_or("info requires a package name")?
                .clone();
            Command::Info(name)
        }
        Some("update") | Some("sync") => Command::Update,
        Some("verify") | Some("check") => Command::Verify,
        Some("--delete-cache") | Some("clean") | Some("clean-cache") => Command::Clean,
        Some("help") => Command::Help,
        Some(other) => return Err(format!("unknown command: {other}")),
    };

    Ok(Cli { config, command })
}
