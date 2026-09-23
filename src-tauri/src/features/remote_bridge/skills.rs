use super::{ssh, tool_adapter::RemoteToolId, BridgeResult, Status};
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, UNIX_EPOCH},
};

const MAX_SKILLS_PER_TOOL: usize = 64;
const MAX_FILES_PER_SKILL: usize = 256;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SKILL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_STORE_BYTES: u64 = 32 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillProjectionState {
    NotSynced,
    Synced,
    LocalChanged,
    Conflict,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub id: String,
    pub tool: RemoteToolId,
    pub name: String,
    pub hash: String,
    pub file_count: usize,
    pub total_size: u64,
    pub state: SkillProjectionState,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SkillFile {
    pub local_path: PathBuf,
    pub relative_path: String,
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct LocalSkill {
    pub view: SkillView,
    pub files: Vec<SkillFile>,
    pub manifest: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectionStore {
    #[serde(default)]
    enabled: Vec<String>,
    #[serde(default)]
    projections: Vec<StoredProjection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProjection {
    id: String,
    tool: RemoteToolId,
    name: String,
    hash: String,
    file_count: usize,
    total_size: u64,
}

#[derive(Default)]
struct RuntimeState {
    target_id: Option<String>,
    stamps: BTreeMap<String, String>,
    states: BTreeMap<String, SkillProjectionState>,
}

static RUNTIME: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
static MONITOR: OnceLock<()> = OnceLock::new();
static OPERATIONS: OnceLock<Mutex<()>> = OnceLock::new();

fn runtime() -> &'static Mutex<RuntimeState> {
    RUNTIME.get_or_init(|| Mutex::new(RuntimeState::default()))
}

fn operations() -> &'static Mutex<()> {
    OPERATIONS.get_or_init(|| Mutex::new(()))
}

fn store_path() -> BridgeResult<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv").join("remote-skills.json"))
        .ok_or_else(|| "stateUnavailable".into())
}

fn load_store() -> BridgeResult<ProjectionStore> {
    let path = store_path()?;
    let Some(bytes) =
        local_file::safe_read(&path, MAX_STORE_BYTES).map_err(|_| "stateUnavailable")?
    else {
        return Ok(ProjectionStore::default());
    };
    let mut store: ProjectionStore =
        serde_json::from_slice(&bytes).map_err(|_| "stateUnavailable")?;
    if store.enabled.len() > MAX_SKILLS_PER_TOOL * 2 {
        return Err("stateUnavailable".into());
    }
    if store.projections.len() > MAX_SKILLS_PER_TOOL * 2 {
        return Err("stateUnavailable".into());
    }
    store.enabled.sort();
    store.enabled.dedup();
    if store.enabled.iter().any(|id| parse_id(id).is_err()) {
        return Err("stateUnavailable".into());
    }
    for projection in &store.projections {
        let (tool, name) = parse_id(&projection.id)?;
        if tool != projection.tool
            || name != projection.name
            || projection.hash.len() != 64
            || !projection.hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || projection.file_count > MAX_FILES_PER_SKILL
            || projection.total_size > MAX_SKILL_BYTES
        {
            return Err("stateUnavailable".into());
        }
    }
    Ok(store)
}

fn persist_store(store: &ProjectionStore) -> BridgeResult<()> {
    let path = store_path()?;
    fs::create_dir_all(path.parent().ok_or("stateUnavailable")?).map_err(|_| "stateUnavailable")?;
    let bytes = serde_json::to_vec_pretty(store).map_err(|_| "stateUnavailable")?;
    local_file::atomic_write(&path, &bytes, "remote-skills").map_err(|_| "stateUnavailable".into())
}

fn parse_id(id: &str) -> BridgeResult<(RemoteToolId, &str)> {
    let (tool, name) = id.split_once('|').ok_or("invalidRequest")?;
    let tool = match tool {
        "codex" => RemoteToolId::Codex,
        "claude" => RemoteToolId::Claude,
        _ => return Err("invalidRequest".into()),
    };
    if !safe_component(name) {
        return Err("invalidRequest".into());
    }
    Ok((tool, name))
}

fn connected_target() -> BridgeResult<String> {
    let state = super::lock()?;
    if state.child.is_none() || state.summary.status != Status::Connected {
        return Err("bridgeUnavailable".into());
    }
    state
        .summary
        .target
        .as_ref()
        .map(|target| target.id.clone())
        .ok_or_else(|| "bridgeUnavailable".into())
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with(".proxyenv-")
}

fn projectable_skill_name(tool: RemoteToolId, value: &str) -> bool {
    safe_component(value) && !(tool == RemoteToolId::Codex && value == ".system")
}

fn root(tool: RemoteToolId) -> BridgeResult<PathBuf> {
    let home = dirs::home_dir().ok_or("stateUnavailable")?;
    let configured = match tool {
        RemoteToolId::Codex => std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex")),
        RemoteToolId::Claude => std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude")),
    };
    if !configured.is_absolute() || !configured.starts_with(&home) {
        return Err("customHome".into());
    }
    Ok(configured.join("skills"))
}

fn collect_files(
    skill_root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<SkillFile>,
    total_size: &mut u64,
) -> BridgeResult<()> {
    if depth > MAX_DEPTH {
        return Err("localSkillUnsafe".into());
    }
    let metadata = fs::symlink_metadata(directory).map_err(|_| "localSkillUnsafe")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("localSkillUnsafe".into());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| "localSkillUnsafe")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "localSkillUnsafe")?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_name = entry.file_name();
        let file_name = file_name.to_str().ok_or("localSkillUnsafe")?;
        if !safe_component(file_name) {
            return Err("localSkillUnsafe".into());
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| "localSkillUnsafe")?;
        if metadata.file_type().is_symlink() {
            return Err("localSkillUnsafe".into());
        }
        if metadata.is_dir() {
            collect_files(skill_root, &path, depth + 1, files, total_size)?;
            continue;
        }
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err("localSkillUnsafe".into());
        }
        if files.len() >= MAX_FILES_PER_SKILL {
            return Err("localSkillUnsafe".into());
        }
        *total_size = total_size
            .checked_add(metadata.len())
            .filter(|size| *size <= MAX_SKILL_BYTES)
            .ok_or("localSkillUnsafe")?;
        let bytes = fs::read(&path).map_err(|_| "localSkillUnsafe")?;
        if bytes.len() as u64 != metadata.len() {
            return Err("localSkillChanged".into());
        }
        let relative = path
            .strip_prefix(skill_root)
            .map_err(|_| "localSkillUnsafe")?;
        let components = relative
            .iter()
            .map(|component| {
                component
                    .to_str()
                    .ok_or_else(|| "localSkillUnsafe".to_owned())
            })
            .collect::<BridgeResult<Vec<_>>>()?;
        if components.is_empty()
            || components
                .iter()
                .any(|component| !safe_component(component))
        {
            return Err("localSkillUnsafe".into());
        }
        let relative_path = components.join("/");
        files.push(SkillFile {
            local_path: path,
            relative_path,
            hash: hex::encode(Sha256::digest(&bytes)),
            size: metadata.len(),
        });
    }
    Ok(())
}

fn inspect(tool: RemoteToolId, skill_root: &Path) -> BridgeResult<LocalSkill> {
    let name = skill_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| safe_component(name))
        .ok_or("localSkillUnsafe")?
        .to_owned();
    let mut files = Vec::new();
    let mut total_size = 0;
    collect_files(skill_root, skill_root, 0, &mut files, &mut total_size)?;
    if !files.iter().any(|file| file.relative_path == "SKILL.md") {
        return Err("localSkillUnsafe".into());
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let manifest = files
        .iter()
        .map(|file| format!("{}  {}", file.hash, file.relative_path))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut digest = Sha256::new();
    for file in &files {
        digest.update(file.relative_path.as_bytes());
        digest.update([0]);
        digest.update(file.hash.as_bytes());
        digest.update([0]);
        digest.update(file.size.to_le_bytes());
    }
    let hash = hex::encode(digest.finalize());
    Ok(LocalSkill {
        view: SkillView {
            id: format!("{}|{name}", tool.as_str()),
            tool,
            name,
            hash,
            file_count: files.len(),
            total_size,
            state: SkillProjectionState::NotSynced,
            enabled: false,
        },
        files,
        manifest,
    })
}

fn collect_local_skills(
    tool: RemoteToolId,
    root: &Path,
    result: &mut Vec<LocalSkill>,
) -> BridgeResult<()> {
    // Claude does not always have a local Skills directory. Absence means
    // "nothing to project" and must not create ~/.claude/skills as a side effect.
    if !root.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(root).map_err(|_| "localSkillUnsafe")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("localSkillUnsafe".into());
    }
    let mut entries = fs::read_dir(root)
        .map_err(|_| "localSkillUnsafe")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "localSkillUnsafe")?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut count = 0;
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !projectable_skill_name(tool, name) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| "localSkillUnsafe")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        count += 1;
        if count > MAX_SKILLS_PER_TOOL {
            return Err("localSkillUnsafe".into());
        }
        result.push(inspect(tool, &entry.path())?);
    }
    Ok(())
}

pub fn local_skills() -> BridgeResult<Vec<LocalSkill>> {
    let mut result = Vec::new();
    for tool in [RemoteToolId::Codex, RemoteToolId::Claude] {
        collect_local_skills(tool, &root(tool)?, &mut result)?;
    }
    Ok(result)
}

pub fn by_id(id: &str) -> BridgeResult<LocalSkill> {
    local_skills()?
        .into_iter()
        .find(|skill| skill.view.id == id)
        .ok_or_else(|| "localSkillMissing".into())
}

fn directories(skill: &LocalSkill) -> String {
    let mut directories = BTreeSet::new();
    for file in &skill.files {
        let mut components = file.relative_path.split('/').collect::<Vec<_>>();
        components.pop();
        while !components.is_empty() {
            directories.insert(components.join("/"));
            components.pop();
        }
    }
    if directories.is_empty() {
        String::new()
    } else {
        directories.into_iter().collect::<Vec<_>>().join("\n") + "\n"
    }
}

fn request(skill: &LocalSkill, operation: &str) -> serde_json::Value {
    serde_json::json!({
        "operation": operation,
        "tool": skill.view.tool.as_str(),
        "skillName": skill.view.name,
        "skillHash": skill.view.hash,
        "fileCount": skill.view.file_count,
        "totalSize": skill.view.total_size,
        "manifestBase64": super::base64(&skill.manifest),
        "directoriesBase64": super::base64(&directories(skill)),
    })
}

fn stored_request(projection: &StoredProjection, operation: &str) -> serde_json::Value {
    serde_json::json!({
        "operation": operation,
        "tool": projection.tool.as_str(),
        "skillName": projection.name,
        "skillHash": projection.hash,
        "fileCount": projection.file_count,
        "totalSize": projection.total_size,
    })
}

fn stored(skill: &LocalSkill) -> StoredProjection {
    StoredProjection {
        id: skill.view.id.clone(),
        tool: skill.view.tool,
        name: skill.view.name.clone(),
        hash: skill.view.hash.clone(),
        file_count: skill.view.file_count,
        total_size: skill.view.total_size,
    }
}

fn update_stored_projection(skill: &LocalSkill) -> BridgeResult<()> {
    let mut store = load_store()?;
    if !store.enabled.contains(&skill.view.id) {
        return Ok(());
    }
    store
        .projections
        .retain(|projection| projection.id != skill.view.id);
    store.projections.push(stored(skill));
    store
        .projections
        .sort_by(|left, right| left.id.cmp(&right.id));
    persist_store(&store)
}

fn state_from(value: &serde_json::Value) -> BridgeResult<SkillProjectionState> {
    match value["state"].as_str() {
        Some("notSynced") => Ok(SkillProjectionState::NotSynced),
        Some("synced") => Ok(SkillProjectionState::Synced),
        Some("localChanged") => Ok(SkillProjectionState::LocalChanged),
        Some("conflict") => Ok(SkillProjectionState::Conflict),
        _ => Err("remoteFailed".into()),
    }
}

fn remember(id: &str, stamp: Option<String>, state: SkillProjectionState) {
    if let Ok(mut runtime) = runtime().lock() {
        if let Some(stamp) = stamp {
            runtime.stamps.insert(id.to_owned(), stamp);
        }
        runtime.states.insert(id.to_owned(), state);
    }
}

fn project(target: &str, skill: &LocalSkill) -> BridgeResult<()> {
    let _operation = operations().lock().map_err(|_| "stateUnavailable")?;
    ssh::skill_remote(target, &request(skill, "prepare"))?;
    let upload = (|| {
        for file in &skill.files {
            ssh::skill_upload(
                target,
                &file.local_path,
                skill.view.tool.as_str(),
                &skill.view.name,
                &skill.view.hash,
                &file.relative_path,
            )?;
        }
        let value = ssh::skill_remote(target, &request(skill, "apply"))?;
        if state_from(&value)? != SkillProjectionState::Synced {
            return Err("skillVerifyFailed".into());
        }
        Ok(())
    })();
    if upload.is_err() {
        let _ = ssh::skill_remote(target, &request(skill, "cleanup"));
    }
    upload
}

pub fn views() -> BridgeResult<Vec<SkillView>> {
    let store = load_store()?;
    let enabled = store.enabled.iter().cloned().collect::<BTreeSet<_>>();
    let target = connected_target().ok();
    let remembered = runtime()
        .lock()
        .map_err(|_| "stateUnavailable")?
        .states
        .clone();
    let mut views = Vec::new();
    for mut skill in local_skills()? {
        skill.view.enabled = enabled.contains(&skill.view.id);
        skill.view.state = if !skill.view.enabled {
            SkillProjectionState::NotSynced
        } else if let Some(target) = target.as_deref() {
            match ssh::skill_remote(target, &request(&skill, "status")) {
                Ok(value) => state_from(&value).unwrap_or(SkillProjectionState::Unavailable),
                Err(code) if code == "skillConflict" => SkillProjectionState::Conflict,
                Err(_) => remembered
                    .get(&skill.view.id)
                    .copied()
                    .unwrap_or(SkillProjectionState::Unavailable),
            }
        } else {
            SkillProjectionState::Unavailable
        };
        remember(&skill.view.id, None, skill.view.state);
        views.push(skill.view);
    }
    let visible = views
        .iter()
        .map(|view| view.id.clone())
        .collect::<BTreeSet<_>>();
    for projection in store
        .projections
        .into_iter()
        .filter(|projection| enabled.contains(&projection.id) && !visible.contains(&projection.id))
    {
        views.push(SkillView {
            id: projection.id,
            tool: projection.tool,
            name: projection.name,
            hash: projection.hash,
            file_count: projection.file_count,
            total_size: projection.total_size,
            state: SkillProjectionState::Unavailable,
            enabled: true,
        });
    }
    Ok(views)
}

pub fn enable(id: String) -> BridgeResult<SkillView> {
    let skill = by_id(&id)?;
    let target = connected_target()?;
    project(&target, &skill)?;
    let mut store = load_store()?;
    if !store.enabled.contains(&id) {
        store.enabled.push(id.clone());
        store.enabled.sort();
    }
    store.projections.retain(|projection| projection.id != id);
    store.projections.push(stored(&skill));
    store
        .projections
        .sort_by(|left, right| left.id.cmp(&right.id));
    if let Err(code) = persist_store(&store) {
        let _ = ssh::skill_remote(&target, &request(&skill, "remove"));
        return Err(code);
    }
    remember(
        &id,
        metadata_stamp(&skill).ok(),
        SkillProjectionState::Synced,
    );
    let mut view = skill.view;
    view.enabled = true;
    view.state = SkillProjectionState::Synced;
    Ok(view)
}

pub fn disable(id: String) -> BridgeResult<SkillView> {
    let mut store = load_store()?;
    let local = by_id(&id).ok();
    let projection = local
        .as_ref()
        .map(stored)
        .or_else(|| {
            store
                .projections
                .iter()
                .find(|projection| projection.id == id)
                .cloned()
        })
        .ok_or("localSkillMissing")?;
    let target = connected_target()?;
    let _operation = operations().lock().map_err(|_| "stateUnavailable")?;
    let value = ssh::skill_remote(&target, &stored_request(&projection, "remove"))?;
    if state_from(&value)? != SkillProjectionState::NotSynced {
        return Err("remoteFailed".into());
    }
    store.enabled.retain(|enabled| enabled != &id);
    store.projections.retain(|stored| stored.id != id);
    persist_store(&store)?;
    remember(&id, None, SkillProjectionState::NotSynced);
    let mut view = local.map(|skill| skill.view).unwrap_or(SkillView {
        id: projection.id,
        tool: projection.tool,
        name: projection.name,
        hash: projection.hash,
        file_count: projection.file_count,
        total_size: projection.total_size,
        state: SkillProjectionState::NotSynced,
        enabled: false,
    });
    view.enabled = false;
    view.state = SkillProjectionState::NotSynced;
    Ok(view)
}

fn metadata_stamp(skill: &LocalSkill) -> BridgeResult<String> {
    let mut digest = Sha256::new();
    for file in &skill.files {
        let metadata = fs::symlink_metadata(&file.local_path).map_err(|_| "localSkillChanged")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("localSkillUnsafe".into());
        }
        let modified = metadata
            .modified()
            .map_err(|_| "localSkillChanged")?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "localSkillChanged")?;
        digest.update(file.relative_path.as_bytes());
        digest.update(metadata.len().to_le_bytes());
        digest.update(modified.as_secs().to_le_bytes());
        digest.update(modified.subsec_nanos().to_le_bytes());
    }
    Ok(hex::encode(digest.finalize()))
}

fn metadata_stamp_directory(
    skill_root: &Path,
    directory: &Path,
    depth: usize,
    digest: &mut Sha256,
    file_count: &mut usize,
    total_size: &mut u64,
) -> BridgeResult<()> {
    if depth > MAX_DEPTH {
        return Err("localSkillUnsafe".into());
    }
    let metadata = fs::symlink_metadata(directory).map_err(|_| "localSkillChanged")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("localSkillUnsafe".into());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| "localSkillChanged")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "localSkillChanged")?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().ok_or("localSkillUnsafe")?;
        if !safe_component(name) {
            return Err("localSkillUnsafe".into());
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| "localSkillChanged")?;
        if metadata.file_type().is_symlink() {
            return Err("localSkillUnsafe".into());
        }
        if metadata.is_dir() {
            metadata_stamp_directory(skill_root, &path, depth + 1, digest, file_count, total_size)?;
            continue;
        }
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err("localSkillUnsafe".into());
        }
        *file_count += 1;
        if *file_count > MAX_FILES_PER_SKILL {
            return Err("localSkillUnsafe".into());
        }
        *total_size = total_size
            .checked_add(metadata.len())
            .filter(|size| *size <= MAX_SKILL_BYTES)
            .ok_or("localSkillUnsafe")?;
        let relative = path
            .strip_prefix(skill_root)
            .map_err(|_| "localSkillUnsafe")?
            .to_string_lossy();
        let modified = metadata
            .modified()
            .map_err(|_| "localSkillChanged")?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "localSkillChanged")?;
        digest.update(relative.as_bytes());
        digest.update(metadata.len().to_le_bytes());
        digest.update(modified.as_secs().to_le_bytes());
        digest.update(modified.subsec_nanos().to_le_bytes());
    }
    Ok(())
}

fn metadata_stamp_id(id: &str) -> BridgeResult<String> {
    let (tool, name) = parse_id(id)?;
    let skill_root = root(tool)?.join(name);
    let mut digest = Sha256::new();
    let mut file_count = 0;
    let mut total_size = 0;
    metadata_stamp_directory(
        &skill_root,
        &skill_root,
        0,
        &mut digest,
        &mut file_count,
        &mut total_size,
    )?;
    if file_count == 0 || !skill_root.join("SKILL.md").is_file() {
        return Err("localSkillUnsafe".into());
    }
    Ok(hex::encode(digest.finalize()))
}

fn monitor_once() -> BridgeResult<()> {
    let target = connected_target()?;
    let enabled = load_store()?.enabled;
    let target_changed = {
        let mut runtime = runtime().lock().map_err(|_| "stateUnavailable")?;
        let changed = runtime.target_id.as_deref() != Some(target.as_str());
        if changed {
            runtime.target_id = Some(target.clone());
            runtime.stamps.clear();
        }
        changed
    };
    let mut changed = Vec::new();
    for id in enabled {
        let stamp = match metadata_stamp_id(&id) {
            Ok(stamp) => stamp,
            Err(_) => {
                remember(&id, None, SkillProjectionState::Unavailable);
                continue;
            }
        };
        let previous = runtime()
            .lock()
            .map_err(|_| "stateUnavailable")?
            .stamps
            .get(&id)
            .cloned();
        if target_changed || previous.as_deref() != Some(stamp.as_str()) {
            changed.push((id, stamp));
        }
    }
    if changed.is_empty() {
        return Ok(());
    }
    thread::sleep(Duration::from_millis(500));
    for (id, old_stamp) in changed {
        let current_stamp = match metadata_stamp_id(&id) {
            Ok(stamp) => stamp,
            Err(_) => {
                remember(&id, None, SkillProjectionState::Unavailable);
                continue;
            }
        };
        if current_stamp != old_stamp {
            remember(&id, None, SkillProjectionState::LocalChanged);
        }
        let skill = match by_id(&id) {
            Ok(skill) => skill,
            Err(_) => {
                remember(&id, None, SkillProjectionState::Unavailable);
                continue;
            }
        };
        match project(&target, &skill) {
            Ok(()) => {
                let _ = update_stored_projection(&skill);
                remember(&id, Some(current_stamp), SkillProjectionState::Synced);
            }
            Err(code) => remember(
                &id,
                None,
                if code == "skillConflict" {
                    SkillProjectionState::Conflict
                } else {
                    SkillProjectionState::Unavailable
                },
            ),
        }
    }
    Ok(())
}

pub fn start_monitor() {
    if MONITOR.set(()).is_err() {
        return;
    }
    thread::spawn(|| loop {
        thread::sleep(Duration::from_secs(2));
        if monitor_once().is_err() {
            if let Ok(mut runtime) = runtime().lock() {
                runtime.target_id = None;
                runtime.stamps.clear();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_components_reject_traversal_shell_syntax_and_reserved_metadata() {
        for valid in ["skill-name", "SKILL.md", "reference_1"] {
            assert!(safe_component(valid));
        }
        for invalid in ["", ".", "..", "bad name", "a/b", "a;cmd", ".proxyenv-owner"] {
            assert!(!safe_component(invalid), "accepted {invalid}");
        }
        assert!(!projectable_skill_name(RemoteToolId::Codex, ".system"));
        assert!(projectable_skill_name(RemoteToolId::Claude, ".system"));
    }

    #[test]
    fn codex_system_skills_are_ignored_and_a_missing_claude_root_stays_absent() {
        let mut nonce = [0_u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let temporary = std::env::temp_dir().join(format!(
            "proxyenv-skill-discovery-test-{}-{}",
            std::process::id(),
            hex::encode(nonce)
        ));
        let codex_root = temporary.join(".codex/skills");
        let system_root = codex_root.join(".system");
        let user_root = codex_root.join("user-skill");
        let claude_root = temporary.join(".claude/skills");
        fs::create_dir_all(&system_root).unwrap();
        fs::create_dir_all(&user_root).unwrap();
        fs::write(system_root.join("SKILL.md"), "# System\n").unwrap();
        fs::write(user_root.join("SKILL.md"), "# User\n").unwrap();

        let mut skills = Vec::new();
        collect_local_skills(RemoteToolId::Codex, &codex_root, &mut skills).unwrap();
        collect_local_skills(RemoteToolId::Claude, &claude_root, &mut skills).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].view.id, "codex|user-skill");
        assert!(!claude_root.exists());
        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn manifest_hash_is_deterministic_and_requires_skill_manifest() {
        let mut nonce = [0_u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let temporary = std::env::temp_dir().join(format!(
            "proxyenv-skill-test-{}-{}",
            std::process::id(),
            hex::encode(nonce)
        ));
        fs::create_dir_all(temporary.join("references")).unwrap();
        fs::write(temporary.join("SKILL.md"), "# Test\n").unwrap();
        fs::write(temporary.join("references/example.md"), "example\n").unwrap();
        let first = inspect(RemoteToolId::Codex, &temporary).unwrap();
        let second = inspect(RemoteToolId::Codex, &temporary).unwrap();
        assert_eq!(first.view.hash, second.view.hash);
        assert_eq!(first.view.file_count, 2);
        assert!(first.manifest.contains("SKILL.md"));
        fs::remove_file(temporary.join("SKILL.md")).unwrap();
        assert_eq!(
            inspect(RemoteToolId::Codex, &temporary).unwrap_err(),
            "localSkillUnsafe"
        );
        fs::remove_dir_all(&temporary).unwrap();
    }
}
