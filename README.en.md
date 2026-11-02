# Skill Manager

[中文](README.md) | English

A Windows desktop app for managing AI agent skills in one place. The central library is the single source of truth; each agent directory links to it. Built-in Git sync keeps the library in step with a remote repository such as GitHub or Gitee.

Built with Tauri 2, React 19, TypeScript, and Rust.

## Features

- **Central skill library**: stored at `~/.agents/skills` by default. Each skill's `SKILL.md` name and description are scanned and shown with file count, size, and modification time.
- **Multi-agent link management**: Claude Code, Codex, Qoder, Qoder CN, Cursor, Zcode, DeepSeek Harness, Workbuddy, Workbuddy AI, Trae, and Trae CN. Only agents present on the machine are listed.
- **Clear per-agent status**: every skill is marked as **linked**, **copy**, or **missing** for each agent.
- **Safe operations**: create links, replace an independent copy with a link to the central library, or merge copies. Overwrite and delete actions require confirmation.
- **Archive and restore**: move skills you do not need right now into an archive (default `~/.agents/archive`) and restore them later. Permanent deletion goes through the system recycle bin.
- **Install from a repository**: accepts `skill@owner/repo`, `owner/repo`, `https://github.com/owner/repo`, and `file://` URLs. Detects either a root `SKILL.md` or `skills/*/SKILL.md`, installs from a shallow clone, and never copies `.git`.
- **Git sync**: commits local changes, fetches and rebases onto the remote branch, then pushes. On conflict, the sync stops and lists the conflicting files for manual resolution. Force push is never used.
- **Credential safety**: access tokens are written only to Windows Credential Manager, never to the config file or logs.

## Download

Get the Windows x64 installer from [Releases](https://github.com/clxzl/skill-manager/releases/latest):

- `Skill-Manager_<version>_x64-setup.exe` (NSIS, recommended)
- `Skill-Manager_<version>_x64_en-US.msi`

Requirements: Windows 10/11 x64. Windows 10 requires the WebView2 Runtime. The installers are not code-signed, so SmartScreen may warn on first launch.

## Quick start

1. **Check the central library**: on first launch it defaults to `~/.agents/skills`; change it to any existing absolute path in Settings.
2. **Add skills**: copy existing skill folders into the central library, or click **Add skill** and install from a Git repository.
3. **Create links**: on the **Agents** page, select the agents that should see a skill. If an agent already has a real directory with the same name, replace it with a link.
4. **Turn on sync**: on the **Sync** page, enter the remote URL, username, and access token, connect, then click **Sync now**.

### Installing from a repository

Enter any of the following in the **Add skill** dialog:

```text
ponytail@DietrichGebert/ponytail   # install only the ponytail skill
DietrichGebert/ponytail            # install every skill in the repository
https://github.com/DietrichGebert/ponytail
```

A root `SKILL.md` is installed under the repository name; otherwise every `skills/<name>/SKILL.md` is installed. You can also choose **Select local folder…** to import a directory directly.

### How sync works

- Remote URLs may use `https://`, `ssh://`, `git@host:path`, or `file://`. Tokens apply only to HTTPS URLs.
- Sync runs `git add -A` → commit (`sync: 首次推送 N 个技能` or `sync: 更新 N 个技能`) → `fetch` → `rebase origin/<branch>` → `push`.
- On conflict it runs `rebase --abort`, pushes nothing, and reports the conflicting files. Resolve them with an external Git tool and retry.
- If the remote already has history while the local library is empty, the app adopts the remote history instead of overwriting it.

## Development

Requirements:

- Node.js 20+ and pnpm
- Rust stable (edition 2024)
- Git for Windows (runtime dependency for sync)
- WebView2 Runtime

```bash
pnpm install
pnpm tauri dev          # development mode
pnpm tauri build        # build installers

pnpm typecheck          # TypeScript type check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

The first `pnpm tauri build` downloads the NSIS/WiX bundling tools automatically.

## Data and security

| Item | Location |
| --- | --- |
| Central skill library | `%USERPROFILE%\.agents\skills` (configurable) |
| Archive | `%USERPROFILE%\.agents\archive` |
| App config | `%APPDATA%\com.skillmanager.desktop\config.json` |
| Git credentials | Windows Credential Manager |

- Filesystem, Git, and credential operations all live in Rust; paths are canonicalized and validated. Scanning does not follow symlinks.
- Windows junctions are preferred for links. Only individual skills are linked, and existing real directories are never overwritten.
- Removing a link deletes the link only, not the content in the central library. Deleting a skill sends it to the system recycle bin.
- Changes are fetched and rebased before pushing. Force push is never used. Tokens are never written to disk or logs.

## Known limitations

- Windows only.
- Sync requires Git and a working credential manager (Git Credential Manager) on the machine.
- Conflicts must be resolved manually with an external Git tool; the app does not auto-merge.
- There is no background auto-sync; sync is triggered manually from the **Sync** page.

## Project layout

```text
src/                 React frontend (UI, state, invoke wrappers)
src-tauri/src/       Rust backend
  lib.rs             Command registration, config, app state
  scan.rs            Skill scanning, linking, archive, repo install
  sync.rs            Git sync and credential handling
src-tauri/icons/     Application icons
```
