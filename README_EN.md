<div align="center">
  <a href="README.md"><kbd>🌐 中文</kbd></a>
</div>

# GitSync

An interactive Git history browser that visualizes commit graphs, branches, diffs, blame, and more, helping developers understand and manage Git repositories more intuitively.

🌐 Supports macOS, Windows, and most Linux distributions. Built with Rust + Tauri for blazing performance 💥

Current version: 0.3.3

## ✨ Features

### Core Browsing
- **Interactive commit history**: hover highlighting, flashlight effect, and virtual scrolling support 1k+ commits without lag
- **Branch visualization and management**: switch branches with one click, with a blue dot following the current branch
- **Commit detail panel**: view diff, file change statistics, and syntax highlighting
- **Smart Blame**: show the author and time of each line of code
- **File timeline**: view the complete modification history of a single file

### Code Tracing
- **Diff syntax highlighting**: added/removed lines are color-coded, with character-level inline highlighting
- **Side-by-side diff**: compare the code of any two commits side by side
- **Semantic code search**: search code content across all historical changes
- **Diff comparison tool**: quickly compare the differences between two commits or branches

### Repository Analysis
- **Repository health report**: scan for large files, stale branches, and merge conflicts
- **Contributor statistics**: visualize by commit count and lines of code
- **Hot file analysis**: list the top 20 most frequently modified files

### Operations
- **Stash management**: save, view, pop, and delete stashes
- **Interactive rebase**: drag and drop commits to rebase
- **Tag management**: create and view Git tags
- **Remote repository management**: view remote addresses
- **Multi-repository management**: quickly switch between multiple Git repositories

### Advanced Tools
- **Graphical merge conflict resolution**: three-column side-by-side comparison, adopt the current/incoming branch with one click
- **Plugin script extension system**: place custom scripts in `~/.git-tool/scripts/` to run them
- **Collapsible sidebar panels**: group features and expand them on demand
- **Git Hooks manager**: browse, edit, and save Git hook scripts
- **UI management**: dark / light / soft color mode themes, custom backgrounds, flashlight size adjustment, and the MD2 option

### Search & Export
- **Commit filtering**: filter by author, date, and file path
- **Report export**: generate Markdown/HTML analysis reports
- **Changelog generation**: automatically generate a CHANGELOG from commit history

### SQL Mini Database
- **GitSQL query engine**: query Git repositories with SQL-like syntax
- **Multi-table joins**: support JOIN on the `commits` and `file_changes` tables
- **Aggregate queries**: COUNT, SUM, AVG, MAX, MIN + GROUP BY
- **Column aliases**: `SELECT col AS alias`
- **Syntax help**: built-in example queries, query history, and CSV export
- **Friendly error messages**: clear hints when a table or column does not exist

### UI Experience
- **Pseudo-liquid glass UI**: frosted glass effect + custom background images
- **Global cursor glow**: soft light that follows the mouse
- **Command palette**: press `Cmd/Ctrl + K` to quickly access all features
- **Auto-scroll**: click a sidebar item to jump to the corresponding panel, with overlay/replace display modes

## 📥 Download

Get the latest installer from the [Releases](https://github.com/Wojusensei/GitSync/releases) page.

### macOS

[GitSync_0.3.3_aarch64.dmg](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/GitSync_0.3.3_aarch64.dmg)

### Windows

[GitSync_0.3.3_x64-setup.exe](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/GitSync_0.3.3_x64-setup.exe)

### Linux (Debian / Ubuntu)

[GitSync_0.3.3_amd64.deb](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/GitSync_0.3.3_amd64.deb)

### Linux (Fedora / RHEL)

[GitSync-0.3.3-1.x86_64.rpm](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/GitSync-0.3.3-1.x86_64.rpm)

### Linux (generic tar.gz)

[GitSync-0.3.3-linux-x86_64.tar.gz.zip](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/GitSync-0.3.3-linux-x86_64.tar.gz.zip)

### Arch Linux

[gitsync-0.3.3-1-x86_64.pkg.tar.zst](https://github.com/Wojusensei/GitSync/releases/download/v0.3.3/gitsync-0.3.3-1-x86_64.pkg.tar.zst)

## 😋 Installation

### macOS

Double-click the `.dmg` file and drag GitSync into the **Applications** folder.

### Windows

Double-click the `.exe` installer and follow the prompts.

### Linux (Debian / Ubuntu)

```bash
sudo dpkg -i GitSync_0.3.3_amd64.deb
```

### Linux (Fedora / RHEL)

```bash
sudo rpm -ivh GitSync-0.3.3-1.x86_64.rpm
```

### Arch Linux

```bash
sudo pacman -U gitsync-0.3.3-1-x86_64.pkg.tar.zst
```

### Linux (generic tar.gz)

```bash
tar -xzvf GitSync-0.3.3-linux-x86_64.tar.gz
./GitSync
```

## 🚀 Usage

1. Launch GitSync
2. Enter the full path of a local Git repository in the input box at the top (for example `/Users/name/projects/my-repo`), or click the 📁 button (located near the top of the interface) to choose one via a dialog
3. Press Enter or click the **Load** button
4. The main area displays the commit history (30 entries are loaded by default; scrolling to the bottom loads more automatically)
5. The sidebar provides entries for branches, analysis tools, and more
6. Click a commit to view its details; click the button next to a file to view blame or its timeline
7. Press `Cmd/Ctrl + K` to open the command palette and quickly invoke any feature

## 🧪 Local Development and Testing

- Before developing, make sure the current branch is `main`, then run `npm run tauri dev`.
- When testing branch switching, do not use GitSync to open GitSync's own source directory. Clicking a branch switches the source repository to another branch, and Tauri immediately recompiles from that branch, so the currently running version may become old code without the new commands.
- It is recommended to prepare a separate test repository, for example `git clone <any-repo> /tmp/gitsync-test`, then load `/tmp/gitsync-test` in GitSync for testing.

## ❓ FAQ

### `Command checkout_branch not found` when clicking a branch

This is not an issue with the branch switching code in the current `main` version. It usually means the local development build is running an old branch or an old build. The most common cause is opening GitSync's own source directory with GitSync and switching branches, as described above. This switches the source repository to an old branch when you click a branch, and Tauri immediately recompiles from that branch, so the command disappears from the currently running version and causes an immediate error.

To fix it:

1. Run `git checkout main` in the GitSync source directory
2. Run `npm run tauri dev` again
3. Load a separate test repository and verify branch switching

### macOS says "damaged and cannot be opened"

If macOS reports "GitSync is damaged and cannot be opened", run the following commands first:

```bash
sudo xattr -rd com.apple.quarantine /Applications/GitSync.app
codesign --force --deep --sign - /Applications/GitSync.app
open /Applications/GitSync.app
```

When the Apple signing secrets are not configured, the installer is signed ad hoc and macOS may report "cannot verify developer"; right-click the icon and choose "Open". After configuring Apple Developer ID signing and notarization, these steps are no longer needed.

## 🛠 Tech Stack

- **Backend**: Rust (git2, serde, chrono, tauri-plugin-dialog)
- **Frontend**: TypeScript + React + Framer Motion + react-virtuoso
- **Desktop framework**: Tauri v2
- **Cross-platform builds**: native compilation on macOS; installers for other operating systems are generated via cross compilation and GitHub Actions

## 🤝 Contributing

We welcome contributions to GitSync through Issues and Pull Requests, including bug fixes, new features, documentation improvements, UI polish, and more.

Please make sure the code follows the project style and run `cargo clippy` and `npm run build` before submitting a PR.

**Submit an Issue**

Search existing issues first to avoid duplicates. Please include as much of the following information as possible:

- Problem description: what happened and what was expected; include detailed logs or even screen recordings if possible
- Reproduction steps: the complete steps from launching the app to reproducing the problem
- Environment info: operating system, GitSync version, Node/Rust versions
- Repository info: whether the affected Git repository is public, its size, and whether it contains submodules or large files
- Log info: terminal output, error messages, and errors returned by Git/GitSync
- Screenshots or screen recordings: helpful for diagnosing UI and interaction issues

**Submit a Pull Request**

- Fork the repository and develop on a new branch; it is recommended to use feat/, fix/, or docs/ prefixes for branch names to make branch management easier
- Keep the changes focused: one PR should fix/improve one known issue or be associated with one Issue
- Describe the motivation, changes, and testing approach in the PR description
- If the PR fixes an Issue, link it in the description

**Code Style**

- Rust code follows rustfmt and clippy
- The frontend keeps the existing TypeScript + React style, reuses existing components and dependencies as much as possible, and does not introduce new frameworks; if it is truly necessary, explain the need in detail to the repository maintainer
- Keep UI copy concise and consistent; error messages may be in Chinese or English and should match the existing style

**Plugin Scripts**

Contributions of plugin scripts are welcome. Place custom scripts in `~/.git-tool/scripts/` to run them. When there are enough plugins, a plugin contribution system and unified plugin management will be considered.

**Arch Linux**

GitSync provides an official Arch Linux package. If you would like to improve the AUR experience, such as submitting a PKGBUILD to AUR, adding more architectures, or improving packaging scripts, PRs are welcome.

## 📄 License

[MIT License](https://opensource.org/licenses/MIT)
