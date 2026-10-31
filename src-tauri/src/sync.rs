use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde::Serialize;

use crate::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub git_available: bool,
    pub repo: bool,
    pub remote: Option<String>,
    pub branch: String,
    pub dirty: u32,
    pub ahead: u32,
    pub behind: u32,
    /// 由配置文件记录，避免每次刷新都去查询凭据管理器
    pub credential: bool,
    pub remote_user: Option<String>,
    pub commits: Vec<CommitInfo>,
}

impl SyncStatus {
    fn empty() -> Self {
        Self {
            git_available: false,
            repo: false,
            remote: None,
            branch: "main".into(),
            dirty: 0,
            ahead: 0,
            behind: 0,
            credential: false,
            remote_user: None,
            commits: Vec::new(),
        }
    }
}

/// 每一项 git 命令都强制只用 Windows 凭据管理器，凭据不落盘
fn command_base() -> Command {
    let mut command = Command::new("git");
    command
        .args(["-c", "credential.helper=", "-c", "credential.helper=manager"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn git(root: &Path, args: &[&str]) -> Command {
    let mut command = command_base();
    command.arg("-C").arg(root).args(args);
    command
}

fn output(root: &Path, args: &[&str]) -> Result<Output, AppError> {
    git(root, args)
        .output()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))
}

fn run(root: &Path, args: &[&str]) -> Result<String, AppError> {
    let out = output(root, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(git_error(&out))
    }
}

fn last_line(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("Git 命令失败")
        .to_string()
}

fn git_error(out: &Output) -> AppError {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let line = last_line(out);
    let message = if stderr.contains("terminal prompts disabled")
        || stderr.contains("could not read Username")
        || stderr.contains("Authentication failed")
        || stderr.contains("401")
    {
        format!("认证失败，请检查访问令牌与用户名（{line}）")
    } else if stderr.contains("Could not resolve host")
        || stderr.contains("unable to access")
        || stderr.contains("Failed to connect")
    {
        format!("无法连接远端仓库，请检查网络与地址（{line}）")
    } else {
        line
    };
    AppError::Msg(message)
}

pub fn git_available() -> bool {
    command_base()
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn is_repo(root: &Path) -> bool {
    root.join(".git").is_dir()
}

fn has_head(root: &Path) -> bool {
    output(root, &["rev-parse", "--verify", "HEAD"]).is_ok_and(|out| out.status.success())
}

fn remote_url(root: &Path) -> Option<String> {
    run(root, &["remote", "get-url", "origin"])
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn current_branch(root: &Path) -> Option<String> {
    run(root, &["symbolic-ref", "--short", "HEAD"])
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn remote_branch_name(root: &Path, current: Option<&str>) -> Option<String> {
    let out = run(
        root,
        &["for-each-ref", "--format=%(refname:short)", "refs/remotes/origin"],
    )
    .ok()?;
    let mut names: Vec<String> = out
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("origin/"))
        .filter(|name| *name != "HEAD")
        .map(str::to_string)
        .collect();
    if let Some(current) = current
        && names.iter().any(|name| name == current)
    {
        return Some(current.to_string());
    }
    if names.len() == 1 {
        return names.pop();
    }
    if names.iter().any(|name| name == "main") {
        return Some("main".into());
    }
    if names.iter().any(|name| name == "master") {
        return Some("master".into());
    }
    names.sort();
    names.into_iter().next()
}

fn ahead_behind(root: &Path, branch: &str) -> Option<(u32, u32)> {
    let out = output(
        root,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("origin/{branch}...HEAD"),
        ],
    )
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    // 左为远端独有（待拉取），右为本地独有（待推送）
    let behind = parts.next()?.parse().ok()?;
    let ahead = parts.next()?.parse().ok()?;
    Some((ahead, behind))
}

fn commits(root: &Path) -> Vec<CommitInfo> {
    run(root, &["log", "-6", "--format=%h%x1f%ct%x1f%s"])
        .ok()
        .map(|text| {
            text.lines()
                .filter_map(|line| {
                    let mut fields = line.split('\u{1f}');
                    Some(CommitInfo {
                        hash: fields.next()?.to_string(),
                        time: fields.next()?.trim().parse().ok()?,
                        message: fields.next().unwrap_or_default().to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn status(root: &Path) -> SyncStatus {
    let mut status = SyncStatus::empty();
    if !git_available() {
        return status;
    }
    status.git_available = true;
    if !is_repo(root) {
        return status;
    }
    status.repo = true;
    status.remote = remote_url(root);
    status.branch = current_branch(root).unwrap_or_else(|| "main".into());
    status.dirty = run(root, &["status", "--porcelain"])
        .ok()
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count() as u32)
        .unwrap_or(0);
    if let Some((ahead, behind)) = ahead_behind(root, &status.branch) {
        status.ahead = ahead;
        status.behind = behind;
    }
    status.commits = commits(root);
    status
}

fn validate_remote(remote: &str) -> Result<(), AppError> {
    let allowed = remote.starts_with("https://")
        || remote.starts_with("ssh://")
        || remote.starts_with("file://")
        || (remote.starts_with("git@") && remote.contains(':'));
    if !allowed {
        return Err(AppError::Msg(
            "仅支持 https://、ssh://、git@host:path 或 file:// 地址".into(),
        ));
    }
    if remote.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::Msg("仓库地址不能包含空白或控制字符".into()));
    }
    if let Some(authority) = remote.strip_prefix("https://").and_then(|rest| rest.split('/').next())
        && authority.contains('@')
    {
        return Err(AppError::Msg(
            "请不要把令牌写进仓库地址，改用下方的访问令牌".into(),
        ));
    }
    Ok(())
}

fn credential(action: &str, url: &str, username: &str, password: Option<&str>) -> Result<String, AppError> {
    let mut input = format!("url={url}\n");
    if !username.is_empty() {
        input.push_str(&format!("username={username}\n"));
    }
    if let Some(password) = password {
        input.push_str(&format!("password={password}\n"));
    }
    input.push('\n');

    let mut child = command_base()
        .arg("credential")
        .arg(action)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| AppError::Msg("无法写入凭据输入".into()))?
        .write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(AppError::Msg(format!("凭据操作失败：{}", last_line(&out))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn credential_store(url: &str, username: &str, token: &str) -> Result<(), AppError> {
    if username.contains(['\n', '\r']) || token.contains(['\n', '\r']) {
        return Err(AppError::Msg("用户名或令牌包含非法字符".into()));
    }
    let username = if username.is_empty() { "oauth2" } else { username };
    credential("approve", url, username, Some(token))?;
    let stored = credential("fill", url, username, None)
        .ok()
        .and_then(|out| {
            out.lines()
                .find_map(|line| line.strip_prefix("password=").map(str::to_string))
        })
        .is_some_and(|password| password.trim_end_matches('\r') == token);
    if !stored {
        return Err(AppError::Msg(
            "无法把凭据保存到 Windows 凭据管理器，请确认已安装 Git Credential Manager，或改用 SSH 地址".into(),
        ));
    }
    Ok(())
}

fn credential_saved(url: &str) -> bool {
    credential("fill", url, "", None).is_ok_and(|out| {
        out.lines()
            .any(|line| line.starts_with("password=") && line.len() > "password=".len())
    })
}

/// 连接远端：初始化仓库、保存凭据、拉取远端分支；返回 HTTPS 凭据是否可用
pub fn connect(root: &Path, remote: &str, username: &str, token: &str) -> Result<bool, AppError> {
    validate_remote(remote)?;
    if !git_available() {
        return Err(AppError::Msg("未找到 Git，请先安装 Git for Windows".into()));
    }
    fs::create_dir_all(root)?;

    let is_https = remote.starts_with("https://");
    if is_https && !token.is_empty() {
        credential_store(remote, username, token)?;
    }

    if !is_repo(root) {
        run(root, &["init", "-b", "main"])?;
    }

    if remote_url(root).is_some() {
        run(root, &["remote", "set-url", "origin", remote])?;
    } else {
        run(root, &["remote", "add", "origin", remote])?;
    }

    run(root, &["fetch", "origin"])
        .map_err(|error| AppError::Msg(format!("连接失败：{error}")))?;

    // 本地还没有提交时，直接把远端历史接上，工作区里的技能保留为本地改动
    if !has_head(root)
        && let Some(branch) = remote_branch_name(root, current_branch(root).as_deref())
    {
        if current_branch(root).as_deref() != Some(branch.as_str()) {
            run(root, &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")])?;
        }
        run(root, &["reset", "--mixed", &format!("origin/{branch}")])?;
    }

    let credential = if is_https {
        !token.is_empty() || credential_saved(remote)
    } else {
        false
    };
    Ok(credential)
}

fn has_staged_changes(root: &Path) -> Result<bool, AppError> {
    let out = output(root, &["diff", "--cached", "--quiet"])?;
    match out.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(git_error(&out)),
    }
}

pub fn sync_now(root: &Path, skills: usize) -> Result<SyncStatus, AppError> {
    if !git_available() {
        return Err(AppError::Msg("未找到 Git，请先安装 Git for Windows".into()));
    }
    if !is_repo(root) || remote_url(root).is_none() {
        return Err(AppError::Msg("尚未连接远端仓库".into()));
    }
    let branch = current_branch(root).ok_or_else(|| AppError::Msg("无法确定当前分支".into()))?;
    let had_head = has_head(root);

    run(root, &["add", "-A"])?;
    if has_staged_changes(root)? {
        let action = if had_head { "更新" } else { "首次推送" };
        let message = format!("sync: {action} {skills} 个技能");
        run(
            root,
            &[
                "-c",
                "user.name=Skill Manager",
                "-c",
                "user.email=skill-manager@localhost",
                "commit",
                "-m",
                &message,
            ],
        )?;
    } else if !had_head {
        return Err(AppError::Msg("中央库没有可同步的内容".into()));
    }

    run(root, &["fetch", "origin"]).map_err(|error| AppError::Msg(format!("拉取远端失败：{error}")))?;

    if let Some(remote_branch) = remote_branch_name(root, Some(&branch)) {
        let target = format!("origin/{remote_branch}");
        let out = output(root, &["rebase", &target])?;
        if !out.status.success() {
            let conflicts = run(root, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
            let _ = git(root, &["rebase", "--abort"]).output();
            let files: Vec<&str> = conflicts
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect();
            if files.is_empty() {
                return Err(AppError::Msg(format!("合并远端失败：{}", git_error(&out))));
            }
            return Err(AppError::Msg(format!(
                "同步冲突（{}）。已停止本次同步、未推送，远端提交保留在 origin/{remote_branch}，请在 Git 中解决后再试",
                files.join("、")
            )));
        }
    }

    run(root, &["push", "origin", &format!("HEAD:{branch}")])
        .map_err(|error| AppError::Msg(format!("推送到远端失败：{error}")))?;
    Ok(status(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_remote(tmp: &tempfile::TempDir) -> (PathBuf, String) {
        let remote = tmp.path().join("remote.git");
        let out = Command::new("git")
            .args(["init", "--bare", "-b", "main"])
            .arg(&remote)
            .output()
            .unwrap();
        assert!(out.status.success());
        let url = format!("file:///{}", remote.to_string_lossy().replace('\\', "/"));
        (remote, url)
    }

    fn setup_library(tmp: &tempfile::TempDir) -> PathBuf {
        let root = tmp.path().join("central");
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/SKILL.md"), "alpha").unwrap();
        fs::create_dir_all(root.join("beta")).unwrap();
        fs::write(root.join("beta/SKILL.md"), "beta").unwrap();
        root
    }

    fn remote_file(remote: &Path, path: &str) -> String {
        run(remote, &["show", &format!("HEAD:{path}")]).unwrap()
    }

    #[test]
    fn validate_remote_blocks_transport_helpers() {
        assert!(validate_remote("https://github.com/u/repo.git").is_ok());
        assert!(validate_remote("git@github.com:u/repo.git").is_ok());
        assert!(validate_remote("ssh://git@host/u/repo.git").is_ok());
        assert!(validate_remote("ext::sh -c whoami").is_err());
        assert!(validate_remote("-oProxyCommand=calc").is_err());
        assert!(validate_remote("http://host/repo").is_err());
        assert!(validate_remote("https://token@github.com/u/repo.git").is_err());
        assert!(validate_remote("https://host/repo\nusername=x").is_err());
    }

    #[test]
    fn connect_and_sync_round_trip() {
        assert!(git_available(), "测试需要本机安装 Git");
        let tmp = tempfile::tempdir().unwrap();
        let (remote, url) = setup_remote(&tmp);
        let root = setup_library(&tmp);

        connect(&root, &url, "", "").unwrap();
        let status = sync_now(&root, 2).unwrap();
        assert_eq!(status.remote.as_deref(), Some(url.as_str()));
        assert_eq!(status.branch, "main");
        assert_eq!(status.ahead, 0);
        assert_eq!(remote_file(&remote, "alpha/SKILL.md"), "alpha");
        assert_eq!(status.commits[0].message, "sync: 首次推送 2 个技能");

        // 远端有新提交时，本地改动应能变基后推送
        let clone = tmp.path().join("clone");
        let out = Command::new("git")
            .args(["clone", "-q", &url])
            .arg(&clone)
            .output()
            .unwrap();
        assert!(out.status.success());
        fs::write(clone.join("beta/SKILL.md"), "remote-edit").unwrap();
        for args in [
            vec!["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-am", "远端修改"],
            vec!["push", "-q", "origin", "HEAD:main"],
        ] {
            let out = Command::new("git").arg("-C").arg(&clone).args(args).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }

        fs::write(root.join("alpha/SKILL.md"), "local-edit").unwrap();
        let status = sync_now(&root, 2).unwrap();
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(remote_file(&remote, "alpha/SKILL.md"), "local-edit");
        assert_eq!(remote_file(&remote, "beta/SKILL.md"), "remote-edit");
    }
}
