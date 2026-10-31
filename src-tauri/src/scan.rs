use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillStatus {
    Link,
    Copy,
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub abbr: String,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub files: u64,
    pub modified: i64,
    pub agents: BTreeMap<String, SkillStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanLink {
    pub agent: String,
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub home: String,
    pub central_root: String,
    pub archive_root: String,
    pub agents: Vec<AgentInfo>,
    pub skills: Vec<SkillInfo>,
    pub archived: Vec<SkillInfo>,
    pub orphans: Vec<OrphanLink>,
}

pub fn agents(home: &Path, root: &Path) -> Vec<AgentInfo> {
    const CATALOG: [(&str, &str, &str, &str); 11] = [
        ("claude", "Claude Code", "CC", ".claude"),
        ("codex", "Codex", "X", ".codex"),
        ("qoder", "Qoder", "Q", ".qoder"),
        ("qoder-cn", "Qoder CN", "QC", ".qoder-cn"),
        ("cursor", "Cursor", "C", ".cursor"),
        ("zcode", "Zcode", "Z", ".zcode"),
        ("deepseek", "DeepSeek Harness", "DS", ".dsh"),
        ("workbuddy", "Workbuddy", "WB", ".workbuddy"),
        ("workbuddy-ai", "Workbuddy AI", "WA", ".workbuddy-ai"),
        ("trae", "Trae", "T", ".trae"),
        ("trae-cn", "Trae CN", "TC", ".trae-cn"),
    ];
    let central = fs::canonicalize(root).ok();
    CATALOG
        .iter()
        .filter_map(|(id, name, abbr, base)| {
            let base = home.join(base);
            if !base.is_dir() {
                return None;
            }
            let dir = base.join("skills");
            // skills 目录被整个链接到中央库时跳过，避免对中央库自身建链
            if central.is_some() && fs::canonicalize(&dir).ok() == central {
                return None;
            }
            Some(AgentInfo {
                id: id.to_string(),
                name: name.to_string(),
                abbr: abbr.to_string(),
                dir: dir.to_string_lossy().into_owned(),
            })
        })
        .collect()
}

pub fn scan(root: &Path, home: &Path) -> ScanResult {
    let agents = agents(home, root);
    let mut skills = scan_skills(root);
    let archive = archive_dir(root);
    let mut orphans = Vec::new();
    let central = central_index(root);

    for agent in &agents {
        let dir = Path::new(&agent.dir);
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if ft.is_symlink() {
                let managed = fs::canonicalize(&path)
                    .ok()
                    .zip(central.get(&name))
                    .is_some_and(|(target, known)| &target == known);
                if managed {
                    if let Some(skill) = skills.iter_mut().find(|s| s.name == name) {
                        skill.agents.insert(agent.id.clone(), SkillStatus::Link);
                    }
                    continue;
                }
                let target = fs::canonicalize(&path)
                    .or_else(|_| fs::read_link(&path))
                    .map(|p| {
                        let text = p.to_string_lossy().into_owned();
                        text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
                    })
                    .unwrap_or_default();
                orphans.push(OrphanLink {
                    agent: agent.id.clone(),
                    name,
                    target,
                });
            } else if ft.is_dir()
                && let Some(skill) = skills.iter_mut().find(|s| s.name == name)
            {
                skill.agents.insert(agent.id.clone(), SkillStatus::Copy);
            }
        }
    }

    for skill in &mut skills {
        for agent in &agents {
            skill
                .agents
                .entry(agent.id.clone())
                .or_insert(SkillStatus::None);
        }
    }

    ScanResult {
        home: home.to_string_lossy().into_owned(),
        central_root: root.to_string_lossy().into_owned(),
        archive_root: archive.to_string_lossy().into_owned(),
        agents,
        skills,
        archived: scan_skills(&archive),
        orphans,
    }
}

/// 归档目录与中央库同级，不随中央库同步
fn archive_dir(root: &Path) -> PathBuf {
    root.parent()
        .map(|parent| parent.join("archive"))
        .unwrap_or_else(|| root.join("archive"))
}

fn skill_description(path: &Path) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let text = text.trim_start_matches('\u{feff}');
    let mut body = text;
    if let Some(rest) = text.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let lines: Vec<&str> = rest[..end].lines().collect();
        if let Some(i) = lines.iter().position(|line| line.trim().starts_with("description:")) {
            let raw = lines[i].trim().strip_prefix("description:").unwrap_or("").trim();
            // YAML 折叠/块标量：description: > 之后的缩进行才是正文
            let value = if raw.starts_with('>') || raw.starts_with('|') {
                clean_inline(
                    &lines[i + 1..]
                        .iter()
                        .skip_while(|line| line.trim().is_empty())
                        .take_while(|line| line.starts_with(' ') || line.starts_with('\t'))
                        .map(|line| line.trim())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            } else {
                clean_inline(raw)
            };
            if !value.is_empty() {
                return value;
            }
        }
        body = &rest[end + 4..];
    }
    body.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("```")
                && !line.starts_with("<!--")
                && !line.starts_with('>')
                && !line.starts_with('|')
        })
        .map(clean_inline)
        .unwrap_or_default()
}

fn clean_inline(text: &str) -> String {
    text.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn scan_skills(root: &Path) -> Vec<SkillInfo> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut skills: Vec<SkillInfo> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        // 中央库即 Git 工作区，.git 不是技能
        .filter(|e| e.file_name() != ".git")
        .map(|entry| {
            let path = entry.path();
            let (files, size_bytes, modified) = dir_stats(&path);
            SkillInfo {
                name: entry.file_name().to_string_lossy().into_owned(),
                description: skill_description(&path.join("SKILL.md")),
                size_bytes,
                files,
                modified,
                agents: BTreeMap::new(),
            }
        })
        .collect();
    skills.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    skills
}

fn dir_stats(dir: &Path) -> (u64, u64, i64) {
    let mut files = 0;
    let mut size = 0;
    let mut modified = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(entry.path());
                continue;
            }
            files += 1;
            if let Ok(md) = entry.metadata() {
                size += md.len();
                if let Ok(time) = md.modified()
                    && let Ok(since) = time.duration_since(UNIX_EPOCH)
                {
                    modified = modified.max(since.as_secs() as i64);
                }
            }
        }
    }
    (files, size, modified)
}

fn central_index(root: &Path) -> HashMap<String, PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return HashMap::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            fs::canonicalize(entry.path())
                .ok()
                .map(|p| (entry.file_name().to_string_lossy().into_owned(), p))
        })
        .collect()
}

fn child_dir(base: &Path, name: &str) -> Result<PathBuf, AppError> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(AppError::Msg("无效的技能名".into()));
    }
    let canon_base = fs::canonicalize(base)
        .map_err(|_| AppError::Msg(format!("目录不可用：{}", base.display())))?;
    let canon = fs::canonicalize(base.join(name))
        .map_err(|_| AppError::Msg(format!("技能不存在：{name}")))?;
    if canon.parent() != Some(canon_base.as_path()) || !canon.is_dir() {
        return Err(AppError::Msg("技能路径越界".into()));
    }
    Ok(canon)
}

pub fn skill_dir(root: &Path, name: &str) -> Result<PathBuf, AppError> {
    child_dir(root, name)
}

fn find_agent(root: &Path, home: &Path, id: &str) -> Result<AgentInfo, AppError> {
    agents(home, root)
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::Msg(format!("未知 Agent：{id}")))
}

fn make_link(target: &Path, link: &Path) -> Result<(), AppError> {
    #[cfg(windows)]
    {
        junction::create(target, link)
            .map_err(|e| AppError::Msg(format!("创建链接失败：{e}")))
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link).map_err(AppError::from)
    }
}

pub fn create_link(root: &Path, home: &Path, skill: &str, agent: &str) -> Result<(), AppError> {
    let target = skill_dir(root, skill)?;
    let agent = find_agent(root, home, agent)?;
    let dir = Path::new(&agent.dir);
    fs::create_dir_all(dir)?;
    let link = dir.join(skill);
    if fs::symlink_metadata(&link).is_ok() {
        return Err(AppError::Msg("目标位置已有同名条目".into()));
    }
    make_link(&target, &link)
}

pub fn replace_with_link(
    root: &Path,
    home: &Path,
    skill: &str,
    agent: &str,
) -> Result<(), AppError> {
    let target = skill_dir(root, skill)?;
    let agent = find_agent(root, home, agent)?;
    let link = Path::new(&agent.dir).join(skill);
    let md = fs::symlink_metadata(&link).map_err(|_| AppError::Msg("未找到独立副本".into()))?;
    if md.file_type().is_symlink() || !md.is_dir() {
        return Err(AppError::Msg("目标不是独立副本".into()));
    }
    trash::delete(&link).map_err(|e| AppError::Msg(format!("移到回收站失败：{e}")))?;
    make_link(&target, &link)
        .map_err(|e| AppError::Msg(format!("副本已移到回收站，但创建链接失败：{e}")))
}

pub fn merge_copies(root: &Path, home: &Path, skill: &str) -> Result<(), AppError> {
    let target = skill_dir(root, skill)?;
    for agent in agents(home, root) {
        let link = Path::new(&agent.dir).join(skill);
        let Ok(md) = fs::symlink_metadata(&link) else {
            continue;
        };
        if md.file_type().is_symlink() || !md.is_dir() {
            continue;
        }
        trash::delete(&link).map_err(|e| AppError::Msg(format!("移到回收站失败：{e}")))?;
        make_link(&target, &link)
            .map_err(|e| AppError::Msg(format!("副本已移到回收站，但创建链接失败：{e}")))?;
    }
    Ok(())
}

pub fn remove_orphans(root: &Path, home: &Path, agent: &str) -> Result<(), AppError> {
    let agent = find_agent(root, home, agent)?;
    let central = central_index(root);
    let dir = Path::new(&agent.dir);
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let managed = fs::canonicalize(entry.path())
            .ok()
            .zip(central.get(&name))
            .is_some_and(|(target, known)| &target == known);
        if managed {
            continue;
        }
        let path = entry.path();
        fs::remove_dir(&path).or_else(|_| fs::remove_file(&path))?;
    }
    Ok(())
}

pub fn add_skill(root: &Path, source: &str) -> Result<(), AppError> {
    let source = fs::canonicalize(source).map_err(|_| AppError::Msg("来源文件夹不存在".into()))?;
    if !source.is_dir() || !source.join("SKILL.md").is_file() {
        return Err(AppError::Msg("请选择包含 SKILL.md 的技能文件夹".into()));
    }
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Msg("无效的技能文件夹".into()))?;
    install_dir(root, &source, &name)
}

fn install_dir(root: &Path, source: &Path, name: &str) -> Result<(), AppError> {
    let dest = root.join(name);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(AppError::Msg(format!("中央库已有同名技能：{name}")));
    }
    fs::create_dir_all(root)?;
    if let Err(error) = copy_dir(source, &dest) {
        let _ = fs::remove_dir_all(&dest);
        return Err(error);
    }
    Ok(())
}

/// 解析「技能名@owner/repo」、owner/repo 或 https:// 仓库地址，返回（可选技能名, 克隆地址, 仓库名）
fn parse_repo_input(input: &str) -> Result<(Option<String>, String, String), AppError> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::Msg("请输入仓库地址".into()));
    }
    let mut name: Option<String> = None;
    let mut source = input;
    if let Some(at) = input.find('@') {
        let (head, tail) = (&input[..at], &input[at + 1..]);
        let tail_is_repo = tail.starts_with("https://")
            || tail.starts_with("file://")
            || (!tail.contains(':')
                && !tail.contains('@')
                && !tail.starts_with('-')
                && tail.split('/').filter(|s| !s.is_empty()).count() == 2);
        if tail_is_repo && !head.is_empty() && !head.contains('/') {
            name = Some(head.to_string());
            source = tail;
        }
    }
    let (url, repo_name) = if let Some(rest) = source.strip_prefix("https://") {
        if rest.contains('@') {
            return Err(AppError::Msg("请不要把令牌写进仓库地址".into()));
        }
        let mut segments = rest.split('/').filter(|s| !s.is_empty());
        let host = segments.next().unwrap_or_default();
        let owner = segments.next().unwrap_or_default();
        let repo = segments.next().unwrap_or_default().trim_end_matches(".git");
        if host.is_empty() || owner.is_empty() || repo.is_empty() {
            return Err(AppError::Msg("无法识别仓库地址，支持 https:// 链接或 owner/repo".into()));
        }
        (format!("https://{host}/{owner}/{repo}"), repo.to_string())
    } else if source.starts_with("file://") {
        let repo_name = source
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .trim_end_matches(".git")
            .to_string();
        if repo_name.is_empty() {
            return Err(AppError::Msg("无法识别 file:// 仓库名".into()));
        }
        (source.to_string(), repo_name)
    } else if !source.contains("://") {
        let parts: Vec<&str> = source.split('/').collect();
        if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) || source.contains(':') {
            return Err(AppError::Msg("无法识别仓库地址，支持 https:// 链接或 owner/repo".into()));
        }
        let repo = parts[1].trim_end_matches(".git");
        (format!("https://github.com/{}/{}", parts[0], repo), repo.to_string())
    } else {
        return Err(AppError::Msg("仅支持 https:// 的 Git 仓库地址".into()));
    };
    Ok((name, url, repo_name))
}

/// 从 Git 仓库浅克隆并安装技能，返回结果描述
pub fn add_skill_from_repo(root: &Path, input: &str) -> Result<String, AppError> {
    let (preferred, url, repo_name) = parse_repo_input(input)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 + d.as_secs())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("skill-manager-clone-{nanos}"));
    let _ = fs::remove_dir_all(&tmp);
    let out = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet"])
        .arg(&url)
        .arg(&tmp)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output();
    let result = match out {
        Ok(out) if out.status.success() => install_from_clone(root, &tmp, &repo_name, preferred.as_deref()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            let message = stderr
                .lines()
                .map(str::trim)
                .rfind(|line| !line.is_empty())
                .unwrap_or("Git 命令失败");
            Err(AppError::Msg(format!("克隆仓库失败：{message}")))
        }
        Err(_) => Err(AppError::Msg("未找到 Git，请先安装 Git for Windows".into())),
    };
    let _ = fs::remove_dir_all(&tmp);
    result
}

fn install_from_clone(
    root: &Path,
    clone: &Path,
    repo_name: &str,
    preferred: Option<&str>,
) -> Result<String, AppError> {
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    if clone.join("SKILL.md").is_file() {
        candidates.push((repo_name.to_string(), clone.to_path_buf()));
    }
    let skills_dir = clone.join("skills");
    if skills_dir.is_dir() {
        for entry in fs::read_dir(&skills_dir)?.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !candidates.iter().any(|(n, _)| *n == name)
                && clone.join("skills").join(&name).join("SKILL.md").is_file()
            {
                candidates.push((name, entry.path()));
            }
        }
    }
    if candidates.is_empty() {
        return Err(AppError::Msg("仓库里没有找到 SKILL.md".into()));
    }
    let chosen: Vec<(String, PathBuf)> = match preferred {
        Some(name) => vec![candidates
            .iter()
            .find(|(n, _)| n == name)
            .ok_or_else(|| {
                let names: Vec<&str> = candidates.iter().map(|(n, _)| n.as_str()).collect();
                AppError::Msg(format!("仓库里没有技能 {name}，可用：{}", names.join("、")))
            })?
            .clone()],
        None => candidates,
    };
    for (name, _) in &chosen {
        if fs::symlink_metadata(root.join(name)).is_ok() {
            return Err(AppError::Msg(format!("中央库已有同名技能：{name}")));
        }
    }
    for (name, dir) in &chosen {
        install_dir(root, dir, name)?;
    }
    let names: Vec<&str> = chosen.iter().map(|(n, _)| n.as_str()).collect();
    if names.len() == 1 {
        Ok(format!("已安装技能 {}", names[0]))
    } else {
        Ok(format!("已安装 {} 个技能：{}", names.len(), names.join("、")))
    }
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), AppError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() || entry.file_name() == ".git" {
            continue; // 链接可能指向库外，.git 不属于技能内容
        }
        let dest = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

pub fn archive_skill(root: &Path, home: &Path, skill: &str) -> Result<(), AppError> {
    let target = skill_dir(root, skill)?;
    let archive = archive_dir(root);
    fs::create_dir_all(&archive)?;
    let dest = archive.join(skill);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(AppError::Msg(format!("归档中已有同名技能：{skill}")));
    }
    remove_links_to(root, home, skill, &target)?;
    fs::rename(&target, &dest).map_err(|e| AppError::Msg(format!("移动到归档失败：{e}")))
}

fn remove_links_to(root: &Path, home: &Path, skill: &str, target: &Path) -> Result<(), AppError> {
    for agent in agents(home, root) {
        let link = Path::new(&agent.dir).join(skill);
        if fs::canonicalize(&link).ok().as_deref() != Some(target) {
            continue;
        }
        fs::remove_dir(&link).or_else(|_| fs::remove_file(&link))?;
    }
    Ok(())
}

pub fn restore_skill(root: &Path, skill: &str) -> Result<(), AppError> {
    let source = child_dir(&archive_dir(root), skill)?;
    let dest = root.join(skill);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(AppError::Msg(format!("中央库已有同名技能：{skill}")));
    }
    fs::create_dir_all(root)?;
    fs::rename(&source, &dest).map_err(|e| AppError::Msg(format!("恢复失败：{e}")))
}

pub fn delete_archived(root: &Path, skill: &str) -> Result<(), AppError> {
    let target = child_dir(&archive_dir(root), skill)?;
    trash::delete(&target).map_err(|e| AppError::Msg(format!("移到回收站失败：{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(".agents/skills");
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(
            root.join("alpha/SKILL.md"),
            "---\nname: alpha\ndescription: Alpha 描述\n---\n# Alpha\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("beta")).unwrap();
        fs::write(root.join("beta/SKILL.md"), "beta").unwrap();

        let claude = tmp.path().join(".claude/skills");
        fs::create_dir_all(&claude).unwrap();
        make_link(
            &fs::canonicalize(root.join("alpha")).unwrap(),
            &claude.join("alpha"),
        )
        .unwrap();
        fs::create_dir_all(claude.join("beta")).unwrap();
        fs::create_dir_all(tmp.path().join(".qoder/skills")).unwrap();

        let old = tmp.path().join("old/skills/gone");
        fs::create_dir_all(&old).unwrap();
        make_link(&fs::canonicalize(&old).unwrap(), &claude.join("gone")).unwrap();
        tmp
    }

    #[test]
    fn scan_classifies_links_copies_and_orphans() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");
        let result = scan(&root, tmp.path());

        assert_eq!(result.skills.len(), 2);
        let alpha = result.skills.iter().find(|s| s.name == "alpha").unwrap();
        assert_eq!(alpha.agents["claude"], SkillStatus::Link);
        assert_eq!(alpha.description, "Alpha 描述");
        let beta = result.skills.iter().find(|s| s.name == "beta").unwrap();
        assert_eq!(beta.agents["claude"], SkillStatus::Copy);
        assert_eq!(beta.description, "beta");

        assert_eq!(result.orphans.len(), 1);
        assert_eq!(result.orphans[0].name, "gone");
    }

    #[test]
    fn description_handles_yaml_block_scalar() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        fs::write(&path, "---\nname: x\ndescription: >\n  第一行\n  第二行\n---\n# X\n").unwrap();
        assert_eq!(skill_description(&path), "第一行 第二行");
    }

    #[test]
    fn description_skips_markup_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        fs::write(&path, "# T\n\n> 引用\n\n| a | b |\n\n真正的介绍\n").unwrap();
        assert_eq!(skill_description(&path), "真正的介绍");
    }

    #[test]
    fn remove_orphans_keeps_managed_links() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");
        remove_orphans(&root, tmp.path(), "claude").unwrap();

        let claude = tmp.path().join(".claude/skills");
        assert!(!claude.join("gone").exists());
        assert!(claude.join("alpha").exists());
        assert!(scan(&root, tmp.path()).orphans.is_empty());
    }

    #[test]
    fn create_and_replace_links() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");

        create_link(&root, tmp.path(), "alpha", "qoder").unwrap();
        let result = scan(&root, tmp.path());
        assert_eq!(
            result.skills.iter().find(|s| s.name == "alpha").unwrap().agents["qoder"],
            SkillStatus::Link
        );

        replace_with_link(&root, tmp.path(), "beta", "claude").unwrap();
        let result = scan(&root, tmp.path());
        assert_eq!(
            result.skills.iter().find(|s| s.name == "beta").unwrap().agents["claude"],
            SkillStatus::Link
        );
    }

    #[test]
    fn detects_only_supported_agents() {
        let tmp = setup();
        fs::create_dir_all(tmp.path().join(".foo/skills")).unwrap();
        fs::create_dir_all(tmp.path().join(".qoder-cn/skills")).unwrap();
        fs::create_dir_all(tmp.path().join(".trae-cn/skills")).unwrap();
        fs::create_dir_all(tmp.path().join(".trae/skills")).unwrap();
        let root = tmp.path().join(".agents/skills");

        let result = scan(&root, tmp.path());
        let ids: Vec<&str> = result.agents.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["claude", "qoder", "qoder-cn", "trae", "trae-cn"]);

        let cn = result.agents.iter().find(|a| a.id == "qoder-cn").unwrap();
        assert_eq!(cn.name, "Qoder CN");
        assert!(cn.dir.ends_with(".qoder-cn\\skills") || cn.dir.ends_with(".qoder-cn/skills"));
    }

    #[test]
    fn rejects_path_traversal() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");
        assert!(skill_dir(&root, "../skills/alpha").is_err());
        assert!(skill_dir(&root, "..").is_err());
        assert!(create_link(&root, tmp.path(), "alpha", "unknown").is_err());
    }

    #[test]
    fn scan_skips_git_dir_in_library() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let result = scan(&root, tmp.path());
        assert!(result.skills.iter().all(|s| s.name != ".git"));
        assert_eq!(result.skills.len(), 2);
    }

    #[test]
    fn parse_repo_input_variants() {
        let (name, url, repo) = parse_repo_input("https://github.com/a/b").unwrap();
        assert_eq!(name, None);
        assert_eq!(url, "https://github.com/a/b");
        assert_eq!(repo, "b");
        let (_, url, _) = parse_repo_input("https://github.com/a/b.git/").unwrap();
        assert_eq!(url, "https://github.com/a/b");
        let (name, url, _) = parse_repo_input("ponytail@DietrichGebert/ponytail").unwrap();
        assert_eq!(name.as_deref(), Some("ponytail"));
        assert_eq!(url, "https://github.com/DietrichGebert/ponytail");
        let (_, url, _) = parse_repo_input("a/b").unwrap();
        assert_eq!(url, "https://github.com/a/b");
        assert!(parse_repo_input("").is_err());
        assert!(parse_repo_input("ponytail").is_err());
        assert!(parse_repo_input("git@github.com:a/b.git").is_err());
        assert!(parse_repo_input("https://token@github.com/a/b").is_err());
    }

    #[test]
    fn add_skill_from_repo_installs_named_and_all() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("repo");
        fs::create_dir_all(src.join("skills/alpha/ref")).unwrap();
        fs::write(
            src.join("skills/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: A\n---\n",
        )
        .unwrap();
        fs::write(src.join("skills/alpha/ref/x.md"), "x").unwrap();
        fs::create_dir_all(src.join("skills/beta")).unwrap();
        fs::write(src.join("skills/beta/SKILL.md"), "b").unwrap();
        fs::create_dir_all(src.join("skills/not-a-skill")).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "-A"],
            vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"],
        ] {
            let out = Command::new("git").arg("-C").arg(&src).args(args).output().unwrap();
            assert!(out.status.success());
        }
        let url = format!("file:///{}", src.to_string_lossy().replace('\\', "/"));

        let root = tmp.path().join("central");
        let message = add_skill_from_repo(&root, &format!("alpha@{url}")).unwrap();
        assert_eq!(message, "已安装技能 alpha");
        assert!(root.join("alpha/SKILL.md").is_file());
        assert!(root.join("alpha/ref/x.md").is_file());
        assert!(!root.join("alpha/.git").exists());
        assert!(!root.join("beta").exists());
        assert!(add_skill_from_repo(&root, &format!("alpha@{url}")).is_err());

        let root2 = tmp.path().join("central2");
        let message = add_skill_from_repo(&root2, &url).unwrap();
        assert_eq!(message, "已安装 2 个技能：alpha、beta");
        assert!(root2.join("beta/SKILL.md").is_file());
        assert!(!root2.join("not-a-skill").exists());
    }

    #[test]
    fn add_skill_copies_folder_into_library() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(".agents/skills");
        let source = tmp.path().join("incoming/new-skill");
        fs::create_dir_all(source.join("ref")).unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        fs::write(source.join("ref/a.md"), "a").unwrap();

        add_skill(&root, source.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("new-skill/ref/a.md")).unwrap(),
            "a"
        );
        assert!(add_skill(&root, source.to_str().unwrap()).is_err());
        assert!(add_skill(&root, tmp.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn archive_moves_skill_and_removes_links_but_keeps_copies() {
        let tmp = setup();
        let root = tmp.path().join(".agents/skills");
        let claude = tmp.path().join(".claude/skills");

        archive_skill(&root, tmp.path(), "alpha").unwrap();
        assert!(fs::symlink_metadata(claude.join("alpha")).is_err());
        assert!(archive_dir(&root).join("alpha/SKILL.md").is_file());

        let result = scan(&root, tmp.path());
        assert!(result.skills.iter().all(|s| s.name != "alpha"));
        assert_eq!(result.archived.len(), 1);
        assert_eq!(result.archived[0].name, "alpha");
        assert!(result.orphans.iter().all(|o| o.name != "alpha"));

        restore_skill(&root, "alpha").unwrap();
        assert!(root.join("alpha/SKILL.md").is_file());
        assert!(scan(&root, tmp.path()).archived.is_empty());

        archive_skill(&root, tmp.path(), "beta").unwrap();
        assert!(claude.join("beta").is_dir());
        assert!(scan(&root, tmp.path()).archived[0].name == "beta");
    }
}
