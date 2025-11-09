<div align="center">
  <img src="./doc/banner.png" />
</div>
<p align="center">
    <a href="https://discord.gg/PdpX3vSHAC">Discord</a>
    ·
    <a href="https://crates.io/crates/dothub">Crates</a>
</p>
<hr>

A community driven dotfile management platform. This is a golden opportunity for beginners and experts alike to make contributions to the beloved opensource community. Contributing can be as simple as adding your dotfiles to the hub.yml. Help is also wanted to solve issues and create features.

## Installation

### Prequisites

- Make sure you have the latest version of rust installed
```bash
rustup update
```
- Make sure the cargo binary directory is in your PATH

**Note:** You can configure root to use your default profile's cargo binaries too.
```bash
# .zshrc|.bashrc|etc
export PATH=$PATH:/home/<my_user>/.cargo/bin
```
Or simply
```bash
export PATH=$PATH:~/.cargo/bin
```

### Cargo Install

Install via cargo
```bash
cargo install dothub
```

### Manual build
```bash
git clone https://github.com/huncholane/dothub
cd dothub
cargo install --path .
```

### yay

Coming Soon

### apt

Coming Soon

### Learn By Example

This example will teach you how to use my personal nvim.
1. Install the config into dothub
```bash
dothub install https://github.com/huncholane/hygo-nvim
```
This simply clones the repository to `~/.local/share/dothub/hygo-nvim`.

**Note:** You can tell dothub to install the repo as a specific name.
```bash
dothub install https://github.com/huncholane/hygo-nvim best-nvim
```
This will clone the repo into `~/.local/share/dothub/best-nvim`

2. Link to your config path
```bash
dothub link hygo-nvim nvim
```
This deletes whatever you have at `~/.config/nvim` and creates a symbolic link from `~/.local/share/dothub/hygo-nvim` to `~/.config/nvim` make sure you have saved your previous config however you like.

#### More about the example

Make sure you learn all you can about the config you are installing. DotHub will not handle third party setups for you. For example my personal nvim requires you to install [Yazi](https://github.com/sxyazi/yazi), the tui file explorer, so you will likely run into errors whenever you use dotfiles from new people.

Also, people update config files pretty often, so to update yours, simply run `dothub update`. This will go through all of your installed dotfile repos and pull them to reflect the latest changes. **Note:** No feature yet to update specific repos.

## Environment

- **DOTHUB_DIR:** Specifies the path to install dothub repos. Defaults to `~/.local/share/dothub`

This is particularly useful for the root user. If you want the root user to use the same dothub repos as your default user, you can add this to `/root/.zshrc`
```bash
# /root/.zshrc
export DOTHUB_DIR=/home/<default_profile>/.local/share/dothub
```
- **GITHUB_TOKEN:** Your [github personal access token](https://github.com/settings/personal-access-tokens). 

Dothub tries to use the github api to retrieve stars and falls back to a less efficient scraping method. You want to set this to make dothub more efficient when using the base `dothub` command.

## Commands

- **dothub:** Displays all dothub profiles in the yml file located on this repo. To register your config files to dothub, fork the repo, make a feature, and submit a pull request. This is a goldmine for first contributions.
- **dothub install [repo] [optional name]:** Installs a repo to your dothub path.
- **dothub link [name] [config type]:** Deletes old config files and creates a symbolic link from the dothub path to your config type.
- **dothub update:** Updates all of your dothub repos. Individual updates coming soon.
- **dothub active:** Shows all current symbolic links managed by dothub.
- **dothub list:** Shows all installed dothub repos. Currently just shows the names, more info coming soon.
- **dothub remove:** Removes a downloaded repo from the dothub dir.
- **dothub completions [shell type]:** Generates completions for the given shell to stdout.
- **dothub help:** Brings up the help menu.

## Completions

I am just getting into creating completions. These will get better. Contributors thoroughly encouraged.

### ZSH

Add this to your .zshrc file if it doesn't exist. There might be a better way to do this. Contributors thoroughly encouraged.
```bash
# Add dothub completions
mkdir -p ~/.zsh/completions
fpath+=("$HOME/.zsh/completions")
autoload -Uz compinit && compinit
```
After starting a new shell, run
```bash
dothub completions zsh > ~/.zsh/completions
```

### Help wanted for more guidance


## Uninstall

There might be a better way to do this. Idk, haven't put much effort into it. Contributors welcome.
```bash
cargo uninstall dothub
rm -rf ~/.local/share/dothub
```

---
#### Attributions
<a href="https://www.flaticon.com/free-icons/discord" title="discord icons">Discord icons created by Freepik - Flaticon</a>
