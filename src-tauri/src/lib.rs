mod scan;
mod sync;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use scan::ScanResult;
use sync::SyncStatus;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    central_root: String,
    #[serde(default)]
    remote_user: Option<String>,
    #[serde(default)]
    credential: bool,
}

fn decorate(mut status: SyncStatus, config: &Config) -> SyncStatus {
    status.credential = config.credential
        && status
            .remote
            .as_deref()
            .is_some_and(|remote| remote.starts_with("https://"));
    status.remote_user = config.remote_user.clone();
    status
}

struct AppState {
    home: PathBuf,
    config_path: PathBuf,
    config: Mutex<Config>,
}

impl AppState {
    fn root(&self) -> PathBuf {
        PathBuf::from(&self.config.lock().unwrap().central_root)
    }
}

fn load_config(path: &Path, default_root: &Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Config>(&raw).ok())
        .filter(|config| !config.central_root.is_empty())
        .unwrap_or_else(|| Config {
            central_root: default_root.to_string_lossy().into_owned(),
            remote_user: None,
            credential: false,
        })
}

fn save_config(path: &Path, config: &Config) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(config)?)?;
    Ok(())
}

#[tauri::command]
fn scan_library(state: State<AppState>) -> Result<ScanResult, AppError> {
    Ok(scan::scan(&state.root(), &state.home))
}

#[tauri::command]
fn set_central_root(path: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = PathBuf::from(&path);
    if !root.is_absolute() || !root.is_dir() {
        return Err(AppError::Msg("请选择一个存在的绝对路径".into()));
    }
    let mut config = state.config.lock().unwrap().clone();
    config.central_root = path;
    save_config(&state.config_path, &config)?;
    *state.config.lock().unwrap() = config;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn create_link(
    skill: String,
    agent: String,
    state: State<AppState>,
) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::create_link(&root, &state.home, &skill, &agent)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn replace_with_link(
    skill: String,
    agent: String,
    state: State<AppState>,
) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::replace_with_link(&root, &state.home, &skill, &agent)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn merge_copies(skill: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::merge_copies(&root, &state.home, &skill)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn remove_orphans(agent: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::remove_orphans(&root, &state.home, &agent)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn add_skill(source: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::add_skill(&root, &source)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn add_skill_repo(source: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::add_skill_from_repo(&root, &source)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn archive_skill(skill: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::archive_skill(&root, &state.home, &skill)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn restore_skill(skill: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::restore_skill(&root, &skill)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn delete_archived(skill: String, state: State<AppState>) -> Result<ScanResult, AppError> {
    let root = state.root();
    scan::delete_archived(&root, &skill)?;
    Ok(scan::scan(&root, &state.home))
}

#[tauri::command]
fn sync_status(state: State<AppState>) -> SyncStatus {
    let status = sync::status(&state.root());
    decorate(status, &state.config.lock().unwrap())
}

#[tauri::command]
fn connect_remote(
    remote: String,
    username: String,
    token: String,
    state: State<AppState>,
) -> Result<SyncStatus, AppError> {
    let root = state.root();
    let credential = sync::connect(&root, remote.trim(), username.trim(), token.trim())?;
    let config = {
        let mut config = state.config.lock().unwrap();
        config.remote_user = Some(username.trim().to_string()).filter(|value| !value.is_empty());
        config.credential = credential;
        config.clone()
    };
    save_config(&state.config_path, &config)?;
    Ok(decorate(sync::status(&root), &config))
}

#[tauri::command]
fn sync_now(state: State<AppState>) -> Result<SyncStatus, AppError> {
    let root = state.root();
    let skills = scan::scan(&root, &state.home).skills.len();
    let status = sync::sync_now(&root, skills)?;
    Ok(decorate(status, &state.config.lock().unwrap()))
}

#[tauri::command]
fn reveal_skill(skill: String, state: State<AppState>) -> Result<(), AppError> {
    let root = state.root();
    scan::skill_dir(&root, &skill)?;
    tauri_plugin_opener::reveal_item_in_dir(root.join(&skill))
        .map_err(|e| AppError::Msg(format!("打开目录失败：{e}")))
}

#[tauri::command]
fn reveal_agent(agent: String, state: State<AppState>) -> Result<(), AppError> {
    let target = scan::agents(&state.home, &state.root())
        .into_iter()
        .find(|a| a.id == agent)
        .ok_or_else(|| AppError::Msg(format!("未知 Agent：{agent}")))?;
    let dir = PathBuf::from(&target.dir);
    if !dir.is_dir() {
        return Err(AppError::Msg("目录不存在，先安装该 Agent 或创建一个技能链接".into()));
    }
    tauri_plugin_opener::reveal_item_in_dir(&dir)
        .map_err(|e| AppError::Msg(format!("打开目录失败：{e}")))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let home = app.path().home_dir()?;
            let default_root = home.join(".agents").join("skills");
            let config_path = app.path().app_config_dir()?.join("config.json");
            let config = load_config(&config_path, &default_root);
            app.manage(AppState {
                home,
                config_path,
                config: Mutex::new(config),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_library,
            set_central_root,
            create_link,
            replace_with_link,
            merge_copies,
            remove_orphans,
            add_skill,
            add_skill_repo,
            archive_skill,
            restore_skill,
            delete_archived,
            sync_status,
            connect_remote,
            sync_now,
            reveal_skill,
            reveal_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
