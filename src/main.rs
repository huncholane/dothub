use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, shells};
use std::collections::HashMap;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY, modifiers::UTF8_ROUND_CORNERS};
use std::env;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;

const DOTHUB_DIR: &str = "/usr/local/share/dothub";
const DEFAULT_HUB_URL: &str =
    "https://raw.githubusercontent.com/huncholane/dothub/refs/heads/main/hub.yml";
const GH_TOKEN_HELP_URL: &str = "https://github.com/settings/personal-access-tokens";

#[derive(Parser)]
#[command(name = "dothub", about = "Manage dotfile repos and links", version)]
struct Cli {
    /// Optional filter: types to include (e.g. nvim, tmux). Comma-separated or space-separated.
    #[arg(value_name = "TYPE", num_args = 0.., value_delimiter = ',')]
    types: Vec<String>,
    /// Optional override URL to YAML (defaults to https://github.com/hub.yml)
    #[arg(long)]
    url: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone a git repository into the dothub store
    Install(InstallArgs),
    /// Replace ~/.config/<target> with a symlink to a stored repo
    Link(LinkArgs),
    /// Pull latest changes for all stored repos
    Update,
    /// List active links in ~/.config that point into dothub
    Active,
    /// List repositories installed in the dothub store
    List,
    /// Generate shell completions to stdout (bash|zsh|fish|powershell|elvish)
    Completions { shell: Shell },
}

// No separate args struct for hub; top-level args cover it

#[derive(Args)]
struct InstallArgs {
    /// Git repository URL, e.g. https://github.com/hygo-nvim
    repo: String,
}

#[derive(Args)]
struct LinkArgs {
    /// Repository name stored under dothub (e.g. hygo-nvim)
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
        Some(Commands::Install(args)) => cmd_install(&args.repo),
        Some(Commands::Link(args)) => cmd_link(&args.name, &args.target),
        Some(Commands::Update) => cmd_update(),
        Some(Commands::Active) => cmd_active(),
        Some(Commands::List) => cmd_list(),
        Some(Commands::Completions { shell }) => cmd_completions(shell),
        None => cmd_hub(cli.types, cli.url),
    }
}

fn ensure_store_dir() -> Result<()> {
    let path = Path::new(DOTHUB_DIR);
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed creating {} (need sudo?)", DOTHUB_DIR))?;
    }
    Ok(())
}

fn derive_repo_name(repo_url: &str) -> String {
    let trimmed = repo_url.trim_end_matches('/').trim_end_matches(".git");
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_string()
}

fn cmd_install(repo: &str) -> Result<()> {
    ensure_store_dir()?;

    // Determine repo name
    let name = derive_repo_name(repo);
    if name.is_empty() {
        bail!("Could not infer repository name from URL: {}", repo);
    }

    let dest = Path::new(DOTHUB_DIR).join(&name);
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
    let source = Path::new(DOTHUB_DIR).join(name);
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
        std::os::unix::fs::symlink(&source, &target).with_context(|| {
            format!(
                "Failed creating symlink {} -> {}",
                target.display(),
                source.display()
            )
        })?;
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(&source, &target).with_context(|| {
                format!(
                    "Failed creating symlink {} -> {}",
                    target.display(),
                    source.display()
                )
            })?;
        } else {
            std::os::windows::fs::symlink_file(&source, &target).with_context(|| {
                format!(
                    "Failed creating symlink {} -> {}",
                    target.display(),
                    source.display()
                )
            })?;
        }
    }

    println!("Linked {} -> {}", source.display(), target.display());
    Ok(())
}

fn cmd_update() -> Result<()> {
    ensure_store_dir()?;
    if which::which("git").is_err() {
        bail!("git is not installed or not found in PATH");
    }

    let root = Path::new(DOTHUB_DIR);
    let mut updated = 0usize;
    let mut skipped = 0usize;

    for entry in fs::read_dir(root).with_context(|| format!("Reading {}", DOTHUB_DIR))? {
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
            eprintln!(
                "git pull failed in {} with status {}",
                path.display(),
                status
            );
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
        Ok(md) if md.is_dir() => fs::remove_dir_all(path)
            .with_context(|| format!("Removing directory {}", path.display())),
        Ok(_md) => {
            fs::remove_file(path).with_context(|| format!("Removing file {}", path.display()))
        }
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
    for entry in
        fs::read_dir(&config_dir).with_context(|| format!("Reading {}", config_dir.display()))?
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

        if resolved.starts_with(DOTHUB_DIR) {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            found.push((name, resolved));
        }
    }

    if found.is_empty() {
        println!("No active dothub links in ~/.config.");
    } else {
        for (name, target) in found {
            println!("{} -> {}", name, target.display());
        }
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    ensure_store_dir()?;
    let root = Path::new(DOTHUB_DIR);
    let mut repos: Vec<String> = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("Reading {}", DOTHUB_DIR))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        repos.push(name);
    }
    repos.sort();
    if repos.is_empty() {
        println!("No repositories installed in {}.", DOTHUB_DIR);
    } else {
        for r in repos {
            println!("{}", r);
        }
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum FlexEntry {
    Single(String),
    Many(Vec<String>),
}

fn cmd_hub(types: Vec<String>, url: Option<String>) -> Result<()> {
    let url = url.as_deref().unwrap_or(DEFAULT_HUB_URL);
    let yaml = match fetch_text(url) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("\x1b[31mFailed to fetch the hub file. Please ensure you have internet connection.\x1b[0m");
            std::process::exit(1);
        }
    };

    let map: HashMap<String, FlexEntry> =
        serde_yaml::from_str(&yaml).context("Parsing YAML for hub")?;

    let filters: Vec<String> = types.iter().map(|s| s.to_lowercase()).collect();

    // Flatten entries into (type, url)
    let mut items: Vec<(String, String)> = Vec::new();
    for (ty, entry) in map.into_iter() {
        if !filters.is_empty() && !filters.contains(&ty.to_lowercase()) {
            continue;
        }
        match entry {
            FlexEntry::Single(u) => items.push((ty.clone(), u)),
            FlexEntry::Many(v) => {
                for u in v {
                    items.push((ty.clone(), u));
                }
            }
        }
    }

    // Collect stars efficiently (GraphQL when token present; REST fallback otherwise)
    let token = env::var("GITHUB_TOKEN").ok();
    let mut warn_graphql_failed = false;
    // Show a spinner during star fetching
    let spinner_stop = start_spinner("Downloading stars from github..");

    let mut detailed: Vec<(String, String, u64)> = Vec::with_capacity(items.len());
    if let Some(ref t) = token {
        let links_only: Vec<String> = items.iter().map(|(_, l)| l.clone()).collect();
        match github_stars_batch(&links_only, Some(t.as_str())) {
            Ok(stars_map) => {
                for (ty, link) in items {
                    let stars = *stars_map.get(&link).unwrap_or(&0);
                    detailed.push((ty, link, stars));
                }
            }
            Err(_) => {
                warn_graphql_failed = true;
                for (ty, link) in items {
                    let stars = github_stars(&link).unwrap_or(0);
                    detailed.push((ty, link, stars));
                }
            }
        }
    } else {
        for (ty, link) in items {
            let stars = github_stars(&link).unwrap_or(0);
            detailed.push((ty, link, stars));
        }
    }

    spinner_stop.store(true, Ordering::SeqCst);
    // Leave the last line in place; print a newline to cleanly end spinner
    eprintln!("");

    // Sort by stars desc
    detailed.sort_by(|a, b| b.2.cmp(&a.2));

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY).apply_modifier(UTF8_ROUND_CORNERS);
    table.set_header(["#", "Stars", "Installed", "Source"]);

    for (idx, (_ty, link, stars)) in detailed.into_iter().enumerate() {
        let rank = (idx + 1).to_string();
        let name = derive_repo_name(&link);
        let installed = Path::new(DOTHUB_DIR).join(&name).exists();
        let installed_str = if installed { "y" } else { "n" };
        table.add_row(vec![rank, stars.to_string(), installed_str.to_string(), link]);
    }

    println!("{}", table);
    if token.is_none() {
        println!(
            "\x1b[33mTo improve performance, please set your GITHUB_TOKEN environment variable.\nLearn more: {}\x1b[0m",
            GH_TOKEN_HELP_URL
        );
    }
    if warn_graphql_failed {
        println!(
            "\x1b[33mGITHUB_TOKEN detected but GitHub GraphQL failed; falling back to REST.\nLearn more: {}\x1b[0m",
            GH_TOKEN_HELP_URL
        );
    }

    Ok(())
}

fn start_spinner(message: &str) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let msg = message.to_string();
    thread::spawn(move || {
        let frames = ["-", "\\", "|", "/"]; // simple spinner
        let mut i = 0usize;
        // Print initial line
        eprint!("{} {}\r", frames[i % frames.len()], msg);
        let _ = std::io::stderr().flush();
        while !stop_clone.load(Ordering::SeqCst) {
            i = (i + 1) % frames.len();
            eprint!("{} {}\r", frames[i], msg);
            let _ = std::io::stderr().flush();
            thread::sleep(Duration::from_millis(120));
        }
    });
    stop
}

fn fetch_text(url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("dothub/0.1")
        .build()
        .context("building http client")?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {}", url))?;
    if !resp.status().is_success() {
        bail!("HTTP {} for {}", resp.status(), url);
    }
    let text = resp.text().context("reading response body")?;
    Ok(text)
}

fn github_stars(link: &str) -> Result<u64> {
    // Expect forms like https://github.com/owner/repo or git@github.com:owner/repo.git
    let lower = link.to_lowercase();
    if !lower.contains("github.com") {
        bail!("not github");
    }

    // Try to extract owner/repo or infer repo if only owner provided
    let mut try_owner_repo: Option<(String, String)> = None;
    if let Ok(parsed) = url::Url::parse(link) {
        if parsed.domain().unwrap_or("") != "github.com" {
            bail!("not github");
        }
        let mut segs = parsed
            .path_segments()
            .ok_or_else(|| anyhow::anyhow!("no path"))?;
        let owner = segs
            .next()
            .ok_or_else(|| anyhow::anyhow!("no owner"))?
            .to_string();
        if let Some(mut repo) = segs.next() {
            if let Some(stripped) = repo.strip_suffix('.').or_else(|| repo.strip_suffix(".git")) {
                repo = stripped;
            }
            try_owner_repo = Some((owner, repo.to_string()));
        } else {
            // Heuristic: try owner/owner as the repository
            try_owner_repo = Some((owner.clone(), owner));
        }
    } else if let Some(rest) = lower.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            let mut repo = parts[1].to_string();
            if let Some(stripped) = repo.strip_suffix('.').or_else(|| repo.strip_suffix(".git")) {
                repo = stripped.to_string();
            }
            try_owner_repo = Some((parts[0].to_string(), repo));
        } else if parts.len() == 1 {
            let owner = parts[0].to_string();
            try_owner_repo = Some((owner.clone(), owner));
        }
    }

    let (owner, repo) = try_owner_repo.ok_or_else(|| anyhow::anyhow!("unrecognized github url"))?;

    let api = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let client = reqwest::blocking::Client::builder()
        .user_agent("dothub/0.1")
        .build()
        .context("building http client")?;
    let resp = client
        .get(&api)
        .send()
        .with_context(|| format!("GET {}", api))?;
    if !resp.status().is_success() {
        bail!("bad status")
    }
    let v: serde_json::Value = resp.json().context("parsing github json")?;
    let stars = v
        .get("stargazers_count")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    Ok(stars)
}

fn parse_github_owner_repo(link: &str) -> Option<(String, String)> {
    let lower = link.to_lowercase();
    if !lower.contains("github.com") {
        return None;
    }
    if let Ok(parsed) = url::Url::parse(link) {
        if parsed.domain().unwrap_or("") != "github.com" {
            return None;
        }
        let mut segs = parsed.path_segments()?;
        let owner = segs.next()?.to_string();
        if let Some(mut repo) = segs.next() {
            if let Some(stripped) = repo.strip_suffix('.').or_else(|| repo.strip_suffix(".git")) {
                repo = stripped;
            }
            return Some((owner, repo.to_string()));
        } else {
            return Some((owner.clone(), owner));
        }
    } else if let Some(rest) = lower.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            let mut repo = parts[1].to_string();
            if let Some(stripped) = repo.strip_suffix('.').or_else(|| repo.strip_suffix(".git")) {
                repo = stripped.to_string();
            }
            return Some((parts[0].to_string(), repo));
        } else if parts.len() == 1 {
            let owner = parts[0].to_string();
            return Some((owner.clone(), owner));
        }
    }
    None
}

fn github_stars_batch(links: &[String], token: Option<&str>) -> Result<HashMap<String, u64>> {
    let mut entries: Vec<(String, (String, String))> = Vec::new();
    for l in links {
        if let Some((o, r)) = parse_github_owner_repo(l) {
            entries.push((l.clone(), (o, r)));
        }
    }
    if entries.is_empty() {
        return Ok(HashMap::new());
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("dothub/0.1")
        .build()
        .context("building http client")?;

    let mut out: HashMap<String, u64> = HashMap::new();

    for chunk in entries.chunks(50) {
        let mut q = String::from("query { ");
        for (i, (_link, (owner, repo))) in chunk.iter().enumerate() {
            let alias = format!("r{}", i);
            let owner_esc = owner.replace('"', "\\\"");
            let repo_esc = repo.replace('"', "\\\"");
            q.push_str(&format!(
                "{}: repository(owner:\"{}\", name:\"{}\") {{ stargazerCount }} ",
                alias, owner_esc, repo_esc
            ));
        }
        q.push('}');

        let mut req = client
            .post("https://api.github.com/graphql")
            .json(&serde_json::json!({"query": q}));
        if let Some(t) = token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }
        let resp = req.send().context("graphql request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("graphql status {}", resp.status()));
        }
        let v: serde_json::Value = resp.json().context("parse graphql json")?;
        if let Some(data) = v.get("data").and_then(|d| d.as_object()) {
            for (i, (link, _)) in chunk.iter().enumerate() {
                let alias = format!("r{}", i);
                let count = data
                    .get(&alias)
                    .and_then(|obj| obj.get("stargazerCount"))
                    .and_then(|n| n.as_u64())
                    .unwrap_or(0);
                out.insert(link.clone(), count);
            }
        }
    }

    Ok(out)
}
