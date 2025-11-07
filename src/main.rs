use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, shells};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

const DOTMAN_DIR: &str = "/usr/local/share/dotman";

#[derive(Parser)]
#[command(name = "dotman", about = "Manage dotfile repos and links", version)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone a git repository into the dotman store
    Install(InstallArgs),
    /// Replace ~/.config/<target> with a symlink to a stored repo
    Link(LinkArgs),
    /// Pull latest changes for all stored repos
    Update,
    /// List active links in ~/.config that point into dotman
    Active,
    /// List repositories installed in the dotman store
    List,
    /// Generate shell completions to stdout (bash|zsh|fish|powershell|elvish)
    Completions { shell: Shell },
}

#[derive(Args)]
struct InstallArgs {
    /// Git repository URL, e.g. https://github.com/hygo-nvim
    repo: String,
}

#[derive(Args)]
struct LinkArgs {
    /// Repository name stored under dotman (e.g. hygo-nvim)
    name: String,
    /// Target directory name under ~/.config (e.g. nvim, alacritty, fish)
    target: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install(args) => cmd_install(&args.repo),
        Commands::Link(args) => cmd_link(&args.name, &args.target),
        Commands::Update => cmd_update(),
        Commands::Active => cmd_active(),
        Commands::List => cmd_list(),
        Commands::Completions { shell } => cmd_completions(shell),
    }
}

fn ensure_dotman_dir() -> Result<()> {
    let path = Path::new(DOTMAN_DIR);
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed creating {} (need sudo?)", DOTMAN_DIR))?;
    }
    Ok(())
}

fn derive_repo_name(repo_url: &str) -> String {
    let trimmed = repo_url.trim_end_matches('/')
        .trim_end_matches(".git");
    trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn cmd_install(repo: &str) -> Result<()> {
    ensure_dotman_dir()?;

    // Determine repo name
    let name = derive_repo_name(repo);
    if name.is_empty() {
        bail!("Could not infer repository name from URL: {}", repo);
    }

    let dest = Path::new(DOTMAN_DIR).join(&name);
    if dest.exists() {
        println!("Repo already exists: {}", dest.display());
        return Ok(());
    }

    // Ensure git is available
    if which::which("git").is_err() {
        bail!("git is not installed or not found in PATH");
    }

    println!("Cloning {} -> {}", repo, dest.display());
    let status = Command::new("git")
        .args(["clone", repo, dest.to_string_lossy().as_ref()])
        .status()
        .with_context(|| "Failed to spawn git clone")?;

    if !status.success() {
        bail!("git clone failed with status: {}", status);
    }

    println!("Installed {}", name);
    Ok(())
}

fn cmd_link(name: &str, target_name: &str) -> Result<()> {
    let source = Path::new(DOTMAN_DIR).join(name);
    if !source.exists() {
        bail!("Source repo not found: {}", source.display());
    }

    // Target: ~/.config/<target_name>
    let home = dirs::home_dir().context("Unable to determine home directory")?;
    let config_dir = home.join(".config");
    let target = config_dir.join(target_name);

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed creating {}", config_dir.display()))?;
    }

    if target.exists() || symlink_exists(&target) {
        remove_path(&target)
            .with_context(|| format!("Failed removing existing {}", target.display()))?;
    }

    // Create symlink
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source, &target)
            .with_context(|| format!(
                "Failed creating symlink {} -> {}",
                target.display(),
                source.display()
            ))?;
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(&source, &target)
                .with_context(|| format!(
                    "Failed creating symlink {} -> {}",
                    target.display(),
                    source.display()
                ))?;
        } else {
            std::os::windows::fs::symlink_file(&source, &target)
                .with_context(|| format!(
                    "Failed creating symlink {} -> {}",
                    target.display(),
                    source.display()
                ))?;
        }
    }

    println!(
        "Linked {} -> {}",
        source.display(),
        target.display()
    );
    Ok(())
}

fn cmd_update() -> Result<()> {
    ensure_dotman_dir()?;
    if which::which("git").is_err() {
        bail!("git is not installed or not found in PATH");
    }

    let root = Path::new(DOTMAN_DIR);
    let mut updated = 0usize;
    let mut skipped = 0usize;

    for entry in fs::read_dir(root).with_context(|| format!("Reading {}", DOTMAN_DIR))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join(".git").exists() {
            skipped += 1;
            continue;
        }

        println!("Updating {}", path.display());
        let status = Command::new("git")
            .args(["-C", path.to_string_lossy().as_ref(), "pull", "--ff-only"])
            .status()
            .with_context(|| format!("Running git pull in {}", path.display()))?;
        if status.success() {
            updated += 1;
        } else {
            eprintln!("git pull failed in {} with status {}", path.display(), status);
        }
    }

    println!("Updated {} repositories (skipped {}).", updated, skipped);
    Ok(())
}

fn cmd_completions(shell: Shell) -> Result<()> {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    match shell {
        Shell::Bash => generate(shells::Bash, &mut cmd, name, &mut io::stdout()),
        Shell::Zsh => generate(shells::Zsh, &mut cmd, name, &mut io::stdout()),
        Shell::Fish => generate(shells::Fish, &mut cmd, name, &mut io::stdout()),
        Shell::PowerShell => generate(shells::PowerShell, &mut cmd, name, &mut io::stdout()),
        Shell::Elvish => generate(shells::Elvish, &mut cmd, name, &mut io::stdout()),
    }
    Ok(())
}

fn symlink_exists(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(md) => md.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => {
            fs::remove_file(path).with_context(|| format!("Removing symlink {}", path.display()))
        }
        Ok(md) if md.is_dir() => {
            fs::remove_dir_all(path).with_context(|| format!("Removing directory {}", path.display()))
        }
        Ok(_md) => fs::remove_file(path).with_context(|| format!("Removing file {}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("Accessing {}", path.display())),
    }
}

fn cmd_active() -> Result<()> {
    let home = dirs::home_dir().context("Unable to determine home directory")?;
    let config_dir = home.join(".config");
    if !config_dir.exists() {
        println!("No ~/.config directory found.");
        return Ok(());
    }

    let mut found = Vec::new();
    for entry in fs::read_dir(&config_dir)
        .with_context(|| format!("Reading {}", config_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        let md = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !md.file_type().is_symlink() {
            continue;
        }

        let link_target = match fs::read_link(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let abs_target = if link_target.is_absolute() {
            link_target.clone()
        } else {
            path.parent()
                .map(|p| p.join(&link_target))
                .unwrap_or_else(|| link_target.clone())
        };

        let resolved = abs_target.canonicalize().unwrap_or(abs_target.clone());

        if resolved.starts_with(DOTMAN_DIR) {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            found.push((name, resolved));
        }
    }

    if found.is_empty() {
        println!("No active dotman links in ~/.config.");
    } else {
        for (name, target) in found {
            println!("{} -> {}", name, target.display());
        }
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    ensure_dotman_dir()?;
    let root = Path::new(DOTMAN_DIR);
    let mut repos: Vec<String> = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("Reading {}", DOTMAN_DIR))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        repos.push(name);
    }
    repos.sort();
    if repos.is_empty() {
        println!("No repositories installed in {}.", DOTMAN_DIR);
    } else {
        for r in repos { println!("{}", r); }
    }
    Ok(())
}
