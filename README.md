dotman
=====

Small Rust CLI to manage dotfile-like repos under `/usr/local/share/dotman`, and link Neovim config.

Commands
- `dotman install <repo-url>`: Clones the repo into `/usr/local/share/dotman/<name>` where `<name>` is derived from the URL (e.g., `https://github.com/hygo-nvim` -> `hygo-nvim`). Requires write access to `/usr/local/share/dotman` (use sudo if needed).
- `dotman link <name> <target>`: Deletes `~/.config/<target>` (if present) and creates a symlink to `/usr/local/share/dotman/<name>`. This is universal; e.g., `dotman link hygo-nvim nvim`, `dotman link my-fish fish`.
- `dotman update`: Runs `git pull --ff-only` for every git repo inside `/usr/local/share/dotman`.
- `dotman completions <SHELL>`: Prints shell completion script to stdout. Supported shells: bash, zsh, fish, powershell, elvish.

Usage examples
- Install: `dotman install https://github.com/hygo-nvim`
- Link Neovim: `dotman link hygo-nvim nvim`
- Link Fish: `dotman link my-fish fish`
- Update all: `dotman update`

Completions
- Bash: `dotman completions bash > /etc/bash_completion.d/dotman` (may require sudo)
- Zsh: `dotman completions zsh > "${fpath[1]}/_dotman"`
- Fish: `dotman completions fish > ~/.config/fish/completions/dotman.fish`
- Powershell: `dotman completions powershell | Out-String | Set-Content $PROFILE.CurrentUserAllHosts`
- Elvish: `dotman completions elvish > ~/.elvish/lib/dotman.elv`

Notes
- Ensure `git` is installed and in PATH.
- Installing and updating may require `sudo` due to the `/usr/local/share/dotman` location.
