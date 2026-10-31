# AGENTS.md

## 项目

Tauri 2 桌面工具，统一管理本机 AI Agent 的 skill：

- 中央库 `~/.agents/skills/<name>/`（含 `SKILL.md`）是唯一真实副本
- 各 Agent 目录（`~/.claude/skills` 等，可配置）通过链接指向中央库
- 中央库即 Git 工作区，与私有仓库（GitHub / Gitee）双向同步
- 认证支持 PAT / OAuth / SSH Key，凭据只存系统密钥链

## 技术栈

Tauri 2 · React 19 + TypeScript(strict) + Vite · pnpm · Zustand · Tailwind
Rust：`git2`、`keyring`、`serde`、`thiserror`、`notify`
配置用 JSON 持久化，v1 不引入数据库。

## 命令

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
pnpm typecheck && pnpm lint && pnpm test
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test  --manifest-path src-tauri/Cargo.toml
```

## 约定

- 文件系统、Git、密钥操作只在 Rust 侧；前端通过 `invoke` 调用，命令返回 `Result<T, AppError>`。
- 路径必须 canonicalize 后校验在允许根目录内；扫描不跟随符号链接。
- Windows 链接优先用 junction；只链单个 skill；不覆盖已有真实目录；删除只删链接，不删中央库内容。
- 不 force push；推送前先 pull；冲突交给用户处理；凭据不落盘、不进日志。
- 提交信息使用中文，格式 `sync: <描述>`。
- 领域决策不明确时先问用户，不自行扩大范围。

## 当前状态

空仓库，尚未初始化脚手架（`pnpm create tauri-app`）；本机未安装 Rust 工具链。
