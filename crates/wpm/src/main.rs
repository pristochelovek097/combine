mod cli;
mod config;
mod db;
mod download;
mod install;

use std::path::Path;
use std::process::Command;

use cli::Command as Action;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = match cli::parse(std::env::args().skip(1).collect()) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("wpm: {message}");
            return 1;
        }
    };

    match cli.command {
        Action::Help => {
            usage();
            return 0;
        }
        Action::Version => {
            println!(
                "wpm {} (COMBINE package manager)",
                env!("CARGO_PKG_VERSION")
            );
            return 0;
        }
        _ => {}
    }

    let writes_to_disk = !matches!(cli.command, Action::ListInstalled);
    if writes_to_disk && cli.config.root == Path::new("/") && !is_root() {
        eprintln!("[ERROR 10] not start of doas/sudo");
        return 10;
    }

    let result = match &cli.command {
        Action::Add(names) => install::add(&cli.config, names),
        Action::Remove(names) => install::remove(&cli.config, names),
        Action::List => install::list_packages(&cli.config),
        Action::ListInstalled => install::list_installed(&cli.config),
        Action::Search(query) => install::search(&cli.config, query),
        Action::Info(name) => install::info(&cli.config, name),
        Action::Update => install::update(&cli.config).map(|_| ()),
        Action::Verify => install::verify(&cli.config),
        Action::Clean => install::clean(&cli.config),
        Action::Help | Action::Version => unreachable!(),
    };

    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("wpm: {message}");
            1
        }
    }
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|uid| uid.trim() == "0")
        .unwrap_or(false)
}

fn usage() {
    println!(
        "\
wpm {} - COMBINE package manager (gdc: glibc profile)

USAGE:
    wpm [OPTIONS] <COMMAND> [ARGS]

COMMANDS:
    add <pkg>...        Install packages, resolving Depends
    remove <pkg>...     Remove installed packages
    list                Print the raw Packages index
    list -i             List installed packages (no root needed)
    search <query>      Search package names
    info <pkg>          Show package details
    update              Refresh the package index
    verify              Check that installed files still exist
    clean               Clear the download cache
    help                Show this help

OPTIONS:
    --root <dir>        Operate on a rootfs instead of / (for image builds)
    --repo <url>        Override the repository URL
    --no-verify         Skip MD5 verification
    --musl              Allow installing the musl loader (glibc by default)
    --reinstall         Reinstall packages that are already present
    -h, --help          Show this help
    -V, --version       Show the version

LAYOUT:
    binaries -> /usr/bin, libraries -> /usr/lib, cache -> /var/cache/wpm
",
        env!("CARGO_PKG_VERSION")
    );
}
