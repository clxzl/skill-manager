use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::{scan, AppError};

pub const SOURCE_FILE: &str = ".skill-source.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSource {
    pub repo: String,
    #[serde(default)]
    pub subdir: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub repo: String,
    pub subdir: String,
    pub commit: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUpdate {
    pub name: String,
    pub repo: String,
    pub installed: String,
    pub latest: String,
    pub local_changes: Vec<String>,
    pub local_changes_known: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub updates: Vec<SkillUpdate>,
    pub errors: Vec<String>,
}

pub fn read_source(dir: &Path) -> Option<SkillSource> {
    let raw = fs::read_to_string(dir.join(SOURCE_FILE)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn source_info(dir: &Path) -> Option<SourceInfo> {
    read_source(dir)
        .filter(|source| !source.repo.is_empty() && !source.commit.is_empty())
        .map(|source| SourceInfo {
            repo: source.repo,
            subdir: source.subdir,
            commit: source.commit,
        })
}

pub(crate) fn write_source(dir: &Path, source: &SkillSource) -> Result<(), AppError> {
    fs::write(dir.join(SOURCE_FILE), serde_json::to_vec(source)?)?;
    Ok(())
}

fn git() -> Command {
    let mut command = Command::new("git");
    command
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

fn last_line(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("Git 命令失败")
        .to_string()
}

fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn clone_shallow(repo: &str) -> Result<PathBuf, AppError> {
    let tmp = std::env::temp_dir().join(format!("skill-manager-clone-{}", unique_id()));
    let _ = fs::remove_dir_all(&tmp);
    let out = git()
        .args(["clone", "--depth", "1", "--quiet", repo])
        .arg(&tmp)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))?;
    if !out.status.success() {
        let _ = fs::remove_dir_all(&tmp);
        return Err(AppError::Msg(format!("克隆仓库失败：{}", last_line(&out))));
    }
    Ok(tmp)
}

pub(crate) fn head_commit(dir: &Path) -> Result<String, AppError> {
    let out = git()
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))?;
    if !out.status.success() {
        return Err(AppError::Msg(format!("无法读取提交版本：{}", last_line(&out))));
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Msg("无法识别提交版本".into()));
    }
    Ok(sha)
}

fn validate_repo(repo: &str) -> Result<(), AppError> {
    let allowed = repo.starts_with("https://") || repo.starts_with("file://");
    if !allowed {
        return Err(AppError::Msg("仅支持 https:// 或 file:// 来源".into()));
    }
    if repo.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::Msg("来源地址包含非法字符".into()));
    }
    if let Some(authority) = repo.strip_prefix("https://").and_then(|rest| rest.split('/').next())
        && authority.contains('@')
    {
        return Err(AppError::Msg("来源地址不能包含用户名或令牌".into()));
    }
    Ok(())
}

fn repo_head(repo: &str) -> Result<String, AppError> {
    let out = git()
        .args(["ls-remote", repo, "HEAD"])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))?;
    if !out.status.success() {
        return Err(AppError::Msg(last_line(&out)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let sha = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_default();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Msg("远端仓库没有可用分支".into()));
    }
    Ok(sha.to_lowercase())
}

fn collect_files(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        if kind.is_dir() {
            if name != ".git" {
                collect_files(base, &entry.path(), out);
            }
        } else if name != SOURCE_FILE {
            let relative = entry
                .path()
                .strip_prefix(base)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if !relative.is_empty() {
                out.push((relative, entry.path()));
            }
        }
    }
}

fn path_for_git(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.strip_prefix("//?/UNC/") {
        Some(rest) => format!("//{rest}"),
        None => text.strip_prefix("//?/").unwrap_or(&text).to_string(),
    }
}

/// 用 git 的 blob 哈希记录技能文件，后续对比即可发现本地改动。
/// shortcut: 文件名含换行时无法批量哈希，Windows 文件名本身不允许换行。
pub(crate) fn hash_files(dir: &Path) -> Result<BTreeMap<String, String>, AppError> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        return Ok(BTreeMap::new());
    }
    // 传绝对路径而不是 git -C：canonicalize 的 \\?\ 前缀会让 Windows 版 Git 打不开文件
    let mut child = git()
        .args(["hash-object", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::Msg("未找到 Git，请先安装 Git for Windows".into()))?;
    let mut input = String::new();
    for (_, path) in &files {
        input.push_str(&path_for_git(path));
        input.push('\n');
    }
    child
        .stdin
        .take()
        .ok_or_else(|| AppError::Msg("无法写入 Git 输入".into()))?
        .write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(AppError::Msg(format!("读取文件版本失败：{}", last_line(&out))));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let hashes: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if hashes.len() != files.len() {
        return Err(AppError::Msg("读取文件版本失败".into()));
    }
    Ok(files
        .into_iter()
        .zip(hashes)
        .map(|((path, _), hash)| (path, hash.to_string()))
        .collect())
}

fn local_changes(dir: &Path, source: &SkillSource) -> (Vec<String>, bool) {
    if source.files.is_empty() {
        return (Vec::new(), false);
    }
    let Ok(current) = hash_files(dir) else {
        return (Vec::new(), false);
    };
    let mut changes = Vec::new();
    for (path, hash) in &source.files {
        match current.get(path) {
            None => changes.push(format!("删除 {path}")),
            Some(now) if now != hash => changes.push(format!("修改 {path}")),
            _ => {}
        }
    }
    for path in current.keys() {
        if !source.files.contains_key(path) {
            changes.push(format!("新增 {path}"));
        }
    }
    (changes, true)
}

pub fn check_updates(root: &Path) -> UpdateReport {
    let mut report = UpdateReport {
        updates: Vec::new(),
        errors: Vec::new(),
    };
    let Ok(entries) = fs::read_dir(root) else {
        return report;
    };
    let mut by_repo: BTreeMap<String, Vec<(String, SkillSource)>> = BTreeMap::new();
    for entry in entries.flatten() {
        if !scan::is_dir_entry(&entry) || entry.file_name() == ".git" {
            continue;
        }
        // 外部目录链接（如 ego-browser）不归中央库管理，更新会动到库外内容
        if entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            continue;
        }
        let Some(source) = read_source(&entry.path()) else {
            continue;
        };
        if source.repo.is_empty() || source.commit.is_empty() {
            continue;
        }
        by_repo
            .entry(source.repo.clone())
            .or_default()
            .push((entry.file_name().to_string_lossy().into_owned(), source));
    }

    for (repo, items) in by_repo {
        if let Err(error) = validate_repo(&repo) {
            report.errors.push(format!("{repo}：{error}"));
            continue;
        }
        let latest = match repo_head(&repo) {
            Ok(latest) => latest,
            Err(error) => {
                report.errors.push(format!("{repo}：{error}"));
                continue;
            }
        };
        for (name, source) in items {
            if latest == source.commit {
                continue;
            }
            let (local_changes, local_changes_known) =
                local_changes(&root.join(&name), &source);
            report.updates.push(SkillUpdate {
                name,
                repo: repo.clone(),
                installed: source.commit,
                latest: latest.clone(),
                local_changes,
                local_changes_known,
            });
        }
    }
    report.updates.sort_by(|a, b| a.name.cmp(&b.name));
    report
}

fn resolve_subdir(clone: &Path, subdir: &str) -> Result<PathBuf, AppError> {
    if subdir.is_empty() {
        return Ok(clone.to_path_buf());
    }
    let relative = Path::new(subdir);
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::Msg("来源路径无效".into()));
    }
    let dir = fs::canonicalize(clone.join(relative))
        .map_err(|_| AppError::Msg("上游已没有该技能目录".into()))?;
    let base = fs::canonicalize(clone)?;
    if !dir.starts_with(&base) || !dir.join("SKILL.md").is_file() {
        return Err(AppError::Msg("上游已没有该技能目录".into()));
    }
    Ok(dir)
}

fn staging_dir(root: &Path) -> PathBuf {
    let base = root.parent().unwrap_or(root);
    let path = base.join(format!(".skill-manager-tmp-{}", unique_id()));
    let _ = fs::create_dir_all(&path);
    path
}

fn ensure_library_dir(root: &Path, name: &str) -> Result<PathBuf, AppError> {
    let dest = scan::skill_dir(root, name)?;
    if fs::symlink_metadata(root.join(name)).is_ok_and(|md| md.file_type().is_symlink()) {
        return Err(AppError::Msg(
            "该技能是外部目录链接，不在中央库内，无法记录来源或更新".into(),
        ));
    }
    Ok(dest)
}

pub fn update_skill(root: &Path, name: &str) -> Result<String, AppError> {
    let dest = ensure_library_dir(root, name)?;
    let source =
        read_source(&dest).ok_or_else(|| AppError::Msg("该技能没有更新来源".into()))?;
    validate_repo(&source.repo)?;
    if source.commit.is_empty() {
        return Err(AppError::Msg("该技能没有记录安装版本".into()));
    }
    let latest = repo_head(&source.repo)?;
    if latest == source.commit {
        return Ok(format!("{name} 已是最新版本"));
    }

    let clone = clone_shallow(&source.repo)?;
    let staging = staging_dir(root);
    let result = (|| -> Result<(), AppError> {
        let from = resolve_subdir(&clone, &source.subdir)?;
        let staged = staging.join(name);
        scan::copy_dir(&from, &staged)?;
        let commit = head_commit(&clone)?;
        let next = SkillSource {
            repo: source.repo.clone(),
            subdir: source.subdir.clone(),
            commit,
            files: hash_files(&staged).unwrap_or_default(),
        };
        write_source(&staged, &next)?;
        trash::delete(&dest)
            .map_err(|error| AppError::Msg(format!("旧版本移到回收站失败，未更新：{error}")))?;
        fs::rename(&staged, &dest).map_err(|error| {
            AppError::Msg(format!(
                "旧版本已移到回收站，但写入新版本失败：{error}；请从回收站恢复 {name}"
            ))
        })?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&clone);
    let _ = fs::remove_dir_all(&staging);
    result?;
    Ok(format!("已更新 {name}"))
}

pub fn attach_source(root: &Path, name: &str, input: &str) -> Result<String, AppError> {
    let dest = ensure_library_dir(root, name)?;
    if !dest.join("SKILL.md").is_file() {
        return Err(AppError::Msg("技能目录里没有 SKILL.md".into()));
    }
    if read_source(&dest).is_some() {
        return Err(AppError::Msg("该技能已有更新来源".into()));
    }
    let (preferred, repo, repo_name) = scan::parse_repo_input(input)?;
    if let Some(preferred) = preferred
        && preferred != name
    {
        return Err(AppError::Msg(format!(
            "输入里的技能名 {preferred} 与当前技能 {name} 不一致"
        )));
    }
    validate_repo(&repo)?;

    let clone = clone_shallow(&repo)?;
    let result = (|| -> Result<(), AppError> {
        let candidates = scan::repo_candidates(&clone, &repo_name)?;
        if candidates.is_empty() {
            return Err(AppError::Msg("仓库里没有找到 SKILL.md".into()));
        }
        let dir = candidates
            .iter()
            .find(|(candidate, _)| candidate == name)
            .or_else(|| {
                if candidates.len() == 1 {
                    candidates.first()
                } else {
                    None
                }
            })
            .map(|(_, dir)| dir)
            .ok_or_else(|| {
                let names: Vec<&str> = candidates.iter().map(|(n, _)| n.as_str()).collect();
                AppError::Msg(format!("仓库里没有技能 {name}，可用：{}", names.join("、")))
            })?;
        let subdir = dir
            .strip_prefix(&clone)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let source = SkillSource {
            repo: repo.clone(),
            subdir,
            commit: head_commit(&clone)?,
            files: hash_files(&dest)?,
        };
        write_source(&dest, &source)
    })();
    let _ = fs::remove_dir_all(&clone);
    result?;
    Ok(format!("已关联更新来源：{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(dir: &Path, message: &str) {
        for args in [
            vec!["add", "-A"],
            vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", message],
        ] {
            let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }

    fn setup_repo(tmp: &tempfile::TempDir) -> (PathBuf, String) {
        let src = tmp.path().join("upstream");
        fs::create_dir_all(src.join("skills/alpha/ref")).unwrap();
        fs::write(src.join("skills/alpha/SKILL.md"), "v1").unwrap();
        fs::write(src.join("skills/alpha/ref/a.md"), "a").unwrap();
        let out = Command::new("git").args(["init", "-q"]).arg(&src).output().unwrap();
        assert!(out.status.success());
        commit(&src, "v1");
        let url = format!("file:///{}", src.to_string_lossy().replace('\\', "/"));
        (src, url)
    }

    #[test]
    fn installed_skill_reports_and_applies_update() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, url) = setup_repo(&tmp);
        let root = tmp.path().join("central");

        scan::add_skill_from_repo(&root, &format!("alpha@{url}")).unwrap();
        let installed = read_source(&root.join("alpha")).expect("安装时应记录来源");
        assert_eq!(installed.commit, head_commit(&src).unwrap());
        assert_eq!(installed.subdir, "skills/alpha");
        assert!(installed.files.contains_key("SKILL.md"));
        assert!(check_updates(&root).updates.is_empty());

        fs::write(src.join("skills/alpha/SKILL.md"), "v2").unwrap();
        commit(&src, "v2");
        let report = check_updates(&root);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.updates.len(), 1);
        assert!(report.updates[0].local_changes.is_empty());
        assert!(report.updates[0].local_changes_known);

        fs::write(root.join("alpha/SKILL.md"), "local").unwrap();
        let report = check_updates(&root);
        assert!(
            report.updates[0]
                .local_changes
                .iter()
                .any(|change| change.contains("SKILL.md"))
        );

        update_skill(&root, "alpha").unwrap();
        assert_eq!(fs::read_to_string(root.join("alpha/SKILL.md")).unwrap(), "v2");
        assert_eq!(
            read_source(&root.join("alpha")).unwrap().commit,
            head_commit(&src).unwrap()
        );
        assert!(check_updates(&root).updates.is_empty());
        assert!(root.join("alpha/ref/a.md").is_file());
    }

    #[test]
    fn attach_source_records_current_baseline() {
        let tmp = tempfile::tempdir().unwrap();
        let (src, url) = setup_repo(&tmp);
        let root = tmp.path().join("central");
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/SKILL.md"), "v1").unwrap();

        attach_source(&root, "alpha", &url).unwrap();
        let source = read_source(&root.join("alpha")).unwrap();
        assert_eq!(source.repo, url);
        assert_eq!(source.subdir, "skills/alpha");
        assert_eq!(source.commit, head_commit(&src).unwrap());
        assert!(source.files.contains_key("SKILL.md"));
        assert!(check_updates(&root).updates.is_empty());
        assert!(attach_source(&root, "alpha", &url).is_err());
    }

    #[test]
    fn git_paths_drop_verbatim_prefix() {
        assert_eq!(path_for_git(Path::new(r"\\?\C:\a\b")), "C:/a/b");
        assert_eq!(path_for_git(Path::new(r"\\?\UNC\srv\share\a")), "//srv/share/a");
    }

    #[test]
    fn rejects_unsafe_repo_urls() {
        assert!(validate_repo("file:///tmp/repo").is_ok());
        assert!(validate_repo("ext::sh -c whoami").is_err());
        assert!(validate_repo("-oProxyCommand=calc").is_err());
        assert!(validate_repo("https://token@github.com/a/b").is_err());
        assert!(validate_repo("ssh://host/repo").is_err());
    }
}
