# Skill Manager

[中文](README.md) | [English](README.en.md)

统一管理本机 AI Agent Skill 的 Windows 桌面工具：中央库是唯一真实副本，各 Agent 目录通过链接引用；内置 Git 同步，可与 GitHub、Gitee 等远端仓库双向同步。

基于 Tauri 2 + React 19 + TypeScript + Rust。

## 功能

- **中央技能库**：默认位于 `~/.agents/skills`，扫描每个技能的 `SKILL.md` 名称与描述，展示文件数、体积和修改时间。
- **多 Agent 链接管理**：支持 Claude Code、Codex、Qoder、Qoder CN、Cursor、Zcode、DeepSeek Harness、Workbuddy、Workbuddy AI、Trae、Trae CN（仅显示本机已安装的 Agent）。
- **状态一目了然**：每个技能在每个 Agent 下标记为「链接」「独立副本」或「未安装」。
- **安全操作**：一键创建链接、把独立副本替换为中央库链接、合并多份副本；覆盖和删除前均有确认。
- **归档与恢复**：暂不使用的技能移入归档（默认 `~/.agents/archive`），随时恢复；彻底删除会进入系统回收站。
- **从仓库安装**：支持 `技能名@owner/repo`、`owner/repo`、`https://github.com/owner/repo` 和 `file://` 地址；自动识别仓库根目录的 `SKILL.md` 或 `skills/*/SKILL.md`，浅克隆安装且不复制 `.git`。
- **Git 同步**：提交本地变更、拉取远端并 rebase、再推送；冲突时中止本次同步并列出冲突文件，交给用户处理，从不强制推送。
- **自动同步**：在设置中开启后按 5/10/30/60 分钟间隔后台同步（默认关闭），窗口隐藏到托盘后仍按时运行。
- **凭据安全**：访问令牌只写入 Windows 凭据管理器，不保存到配置文件，也不会出现在日志中。

## 下载安装

前往 [Releases](https://github.com/clxzl/skill-manager/releases/latest) 下载 Windows x64 安装包：

- `Skill-Manager_<version>_x64-setup.exe`（NSIS，推荐）
- `Skill-Manager_<version>_x64_en-US.msi`

系统要求：Windows 10/11 x64；Windows 10 需已安装 WebView2 Runtime。安装包未做代码签名，首次运行可能触发 SmartScreen 提示。

## 快速上手

1. **确认中央库**：首次启动默认使用 `~/.agents/skills`，可在「设置」中改为其他绝对路径。
2. **添加技能**：把已有技能文件夹复制进中央库，或点击「添加技能」从 Git 仓库安装。
3. **建立链接**：在「目标 Agent」页为技能勾选 Agent；若该 Agent 下已有同名真实目录，可将其替换为链接。
4. **开启同步**：在「同步」页填写远端仓库地址、用户名和访问令牌，连接后点击「立即同步」；需要后台自动同步时在「设置 › 同步」中打开。

### 从仓库添加技能

在「添加技能」中输入以下任一形式：

```text
ponytail@DietrichGebert/ponytail   # 只安装仓库中的 ponytail 技能
DietrichGebert/ponytail            # 安装仓库内全部技能
https://github.com/DietrichGebert/ponytail
```

仓库根目录有 `SKILL.md` 时按仓库名安装；存在 `skills/<name>/SKILL.md` 时安装其中所有技能。也可点击「选择本地文件夹…」直接导入本地目录。

### Git 同步行为

- 连接时支持 `https://`、`ssh://`、`git@host:path`、`file://` 地址；令牌仅适用于 HTTPS 地址。
- 同步流程：`git add -A` → 提交（`sync: 首次推送 N 个技能` / `sync: 更新 N 个技能`）→ `fetch` → `rebase origin/<branch>` → `push`。
- 发生冲突时执行 `rebase --abort`，本次不推送，并提示冲突文件；请在外部 Git 工具中解决后重试。
- 远端已有历史、本地为空仓库时，会自动采用远端分支历史，不会覆盖远端。
- 自动同步复用同一流程，按固定间隔触发；冲突时同样停止并提醒，不会强推。

## 开发

环境要求：

- Node.js 20+ 与 pnpm
- Rust stable（edition 2024）
- Git for Windows（同步功能运行时依赖）
- WebView2 Runtime

```bash
pnpm install
pnpm tauri dev          # 开发模式
pnpm tauri build        # 构建安装包

pnpm typecheck          # TypeScript 类型检查
pnpm test               # 前端自检脚本
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

`pnpm tauri build` 首次运行会自动下载 NSIS/WiX 打包工具。

## 数据与安全

| 内容 | 位置 |
| --- | --- |
| 中央技能库 | `%USERPROFILE%\.agents\skills`（可配置） |
| 归档 | `%USERPROFILE%\.agents\archive` |
| 应用配置 | `%APPDATA%\com.skillmanager.desktop\config.json` |
| Git 凭据 | Windows 凭据管理器 |

- 文件系统、Git 和凭据操作全部在 Rust 侧完成，路径会 canonicalize 后校验；扫描不跟随符号链接。
- Windows 优先使用 junction 创建链接，只链接单个技能，不覆盖已有真实目录。
- 删除链接只删除链接本身，不会删除中央库内容；删除技能会进入系统回收站。
- 推送前先拉取并 rebase；不执行 force push；令牌不落盘、不进日志。

## 已知限制

- 仅支持 Windows。
- 同步依赖本机安装的 Git，并需要配置好的凭据管理器（Git Credential Manager）。
- 冲突只能由用户在外部 Git 工具中解决，应用不会自动合并。
- 自动同步按固定间隔运行，不做文件级实时监听。

## 目录结构

```text
src/                  React 前端（界面、状态与 invoke 封装）
src-tauri/src/        Rust 后端
  lib.rs              命令注册、配置与应用状态
  scan.rs             技能扫描、链接、归档与仓库安装
  sync.rs             Git 同步与凭据管理
src-tauri/icons/      应用图标
```
