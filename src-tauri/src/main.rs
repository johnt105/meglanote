use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

/// Holds a clip payload if one arrives before the frontend has had a
/// chance to start listening for it (e.g. a cold-start launch via the
/// web clipper) — the frontend asks for it once on load, in addition to
/// listening live for the case where the app is already running.
struct ClipState(Mutex<Option<serde_json::Value>>);

/// Holds the update metadata from the last successful `check_for_update`
/// call, so `install_update` can install it without checking again.
struct UpdateState(Mutex<Option<tauri_plugin_updater::Update>>);

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Note {
    id: String,
    title: String,
    body: String,
    tags: Vec<String>,
    pinned: bool,
    color: String,
    folder: String,
    #[serde(rename = "updatedAt")]
    updated_at: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    #[serde(rename = "notesDir")]
    notes_dir: Option<String>,
}

fn app_config_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MeglaNote");
    let _ = fs::create_dir_all(&dir);
    dir.join("app-config.json")
}

fn load_app_config() -> AppConfig {
    fs::read_to_string(app_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_app_config(cfg: &AppConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(app_config_path(), json).map_err(|e| e.to_string())
}

fn notes_dir() -> PathBuf {
    let cfg = load_app_config();
    let dir = match cfg.notes_dir {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => dirs::document_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("MeglaNote"),
    };
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

fn assets_dir() -> PathBuf {
    let dir = notes_dir().join("assets");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

fn trash_dir() -> PathBuf {
    let dir = notes_dir().join(".trash");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

fn settings_path() -> PathBuf {
    notes_dir().join(".settings.json")
}

fn parse_note_file(raw: &str) -> (String, String, Vec<String>, bool, String) {
    if let Some(stripped) = raw.strip_prefix("---\n") {
        if let Some(end) = stripped.find("\n---\n") {
            let front = &stripped[..end];
            let body = &stripped[end + 5..];
            let mut title = String::new();
            let mut tags: Vec<String> = Vec::new();
            let mut pinned = false;
            let mut color = String::new();
            for line in front.lines() {
                if let Some(rest) = line.strip_prefix("title:") {
                    let rest = rest.trim();
                    title = serde_json::from_str::<String>(rest)
                        .unwrap_or_else(|_| rest.to_string());
                } else if let Some(rest) = line.strip_prefix("tags:") {
                    tags = serde_json::from_str::<Vec<String>>(rest.trim()).unwrap_or_default();
                } else if let Some(rest) = line.strip_prefix("pinned:") {
                    pinned = rest.trim() == "true";
                } else if let Some(rest) = line.strip_prefix("color:") {
                    color = serde_json::from_str::<String>(rest.trim()).unwrap_or_default();
                }
            }
            return (title, body.to_string(), tags, pinned, color);
        }
    }
    (String::new(), raw.to_string(), Vec::new(), false, String::new())
}

fn serialize_note_file(title: &str, body: &str, tags: &[String], pinned: bool, color: &str) -> String {
    format!(
        "---\ntitle: {}\ntags: {}\npinned: {}\ncolor: {}\n---\n{}",
        serde_json::to_string(title).unwrap_or_else(|_| "\"\"".to_string()),
        serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()),
        pinned,
        serde_json::to_string(color).unwrap_or_else(|_| "\"\"".to_string()),
        body
    )
}

/// Turn a note title into a filesystem-safe base filename (no extension).
fn sanitize_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    let limited: String = trimmed.chars().take(100).collect();
    if limited.trim().is_empty() {
        "Untitled".to_string()
    } else {
        limited
    }
}

/// Sanitizes a single folder path *segment* (no "/" splitting) - empty
/// stays empty (root/unfiled).
fn sanitize_folder_segment(segment: &str) -> String {
    let cleaned: String = segment
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    cleaned.trim().chars().take(60).collect()
}

/// Sanitizes a full (possibly nested) folder path like "Work/Clients/Acme":
/// splits on "/", sanitizes each segment, and drops empty ones - so stray
/// slashes or trailing/leading ones don't create odd empty path components.
fn sanitize_folder_path(path: &str) -> String {
    path.split('/')
        .map(sanitize_folder_segment)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Find a free "<base>.md" / "<base> (2).md" / ... path in `dir`, treating
/// `exclude` (the note's current file, if any) as not-a-collision.
fn unique_path(dir: &Path, base: &str, exclude: Option<&Path>) -> PathBuf {
    let mut candidate = dir.join(format!("{}.md", base));
    let mut n = 2;
    loop {
        let occupied = candidate.exists() && exclude.map_or(true, |ex| ex != candidate);
        if !occupied {
            return candidate;
        }
        candidate = dir.join(format!("{} ({}).md", base, n));
        n += 1;
    }
}

/// Scans `root` and every subfolder beneath it, to any depth (skipping
/// hidden dirs and the reserved "assets" folder), collecting notes. A
/// note's `id` is its path relative to `root` with the ".md" stripped —
/// so a note in a nested folder gets an id like "Work/Clients/Acme/Notes".
fn collect_notes(root: &Path, dir: &Path, out: &mut Vec<Note>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "assets" {
                continue;
            }
            collect_notes(root, &path, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let (title, body, tags, pinned, color) = parse_note_file(&raw);
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let id = rel.with_extension("").to_string_lossy().replace('\\', "/");
        let folder = path
            .parent()
            .filter(|p| *p != root)
            .map(|p| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let updated_at = fs::metadata(&path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as f64)
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);
        out.push(Note { id, title, body, tags, pinned, color, folder, updated_at });
    }
}

fn read_notes_recursive(root: &Path) -> Vec<Note> {
    let mut notes = Vec::new();
    collect_notes(root, root, &mut notes);
    notes
}

fn purge_old_trash() {
    let dir = trash_dir();
    let cutoff = SystemTime::now().checked_sub(Duration::from_secs(14 * 24 * 60 * 60));
    let cutoff = match cutoff {
        Some(c) => c,
        None => return,
    };
    for note in read_notes_recursive(&dir) {
        let path = dir.join(format!("{}.md", note.id));
        if let Ok(meta) = fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if modified < cutoff {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

#[tauri::command]
fn list_notes() -> Vec<Note> {
    read_notes_recursive(&notes_dir())
}

#[tauri::command]
fn list_trash() -> Vec<Note> {
    read_notes_recursive(&trash_dir())
}

/// Recursively collects every folder path under `dir` (relative to
/// `root`), to any depth - "Work", then "Work/Clients", etc.
fn collect_folder_paths(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "assets" {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        out.push(rel);
        collect_folder_paths(root, &path, out);
    }
}

#[tauri::command]
fn list_folders() -> Vec<String> {
    let dir = notes_dir();
    let mut folders = Vec::new();
    collect_folder_paths(&dir, &dir, &mut folders);
    folders.sort();
    folders
}

#[tauri::command]
fn create_folder(name: String) -> Result<(), String> {
    let clean = sanitize_folder_path(&name);
    if clean.is_empty() {
        return Err("Folder name can't be empty".to_string());
    }
    fs::create_dir_all(notes_dir().join(&clean)).map_err(|e| e.to_string())
}

/// Renames a folder on disk. Returns the (sanitized) new folder name so the
/// frontend can update its filter/selection to match.
#[tauri::command]
fn rename_folder(old_name: String, new_name: String) -> Result<String, String> {
    let old_clean = sanitize_folder_path(&old_name);
    let new_clean = sanitize_folder_path(&new_name);
    if new_clean.is_empty() {
        return Err("Folder name can't be empty".to_string());
    }
    if new_clean == old_clean {
        return Ok(new_clean);
    }
    if new_clean.starts_with(&format!("{}/", old_clean)) {
        return Err("Can't move a folder inside itself".to_string());
    }
    let dir = notes_dir();
    let old_path = dir.join(&old_clean);
    let new_path = dir.join(&new_clean);
    if !old_path.is_dir() {
        return Err("Folder not found".to_string());
    }
    if new_path.exists() {
        return Err("A folder with that name already exists".to_string());
    }
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())?;
    Ok(new_clean)
}

/// Deletes a folder. Every note inside is sent to Trash first (same as
/// deleting it individually, so it can still be restored within 14 days),
/// then the now-empty folder itself is removed.
#[tauri::command]
fn delete_folder(name: String) -> Result<(), String> {
    let clean = sanitize_folder_path(&name);
    if clean.is_empty() {
        return Err("Invalid folder".to_string());
    }
    let dir = notes_dir();
    let folder_path = dir.join(&clean);
    if folder_path.is_dir() {
        let mut notes_in_folder = Vec::new();
        collect_notes(&dir, &folder_path, &mut notes_in_folder);
        for note in notes_in_folder {
            let _ = delete_note(note.id);
        }
        let _ = fs::remove_dir_all(&folder_path);
    }
    Ok(())
}

/// Saves a note. Renames/moves its file to match the (sanitized) title
/// and folder if either changed. `old_id` is the note's current relative
/// path (no extension), or a not-yet-existing placeholder for a new note.
/// Returns the id the note ends up saved under.
#[tauri::command]
fn save_note(
    old_id: String,
    title: String,
    body: String,
    tags: Vec<String>,
    pinned: bool,
    color: String,
    folder: String,
) -> Result<String, String> {
    let dir = notes_dir();
    let old_path = dir.join(format!("{}.md", old_id));
    let old_exists = old_path.exists();
    let desired_base = sanitize_title(&title);
    let folder_clean = sanitize_folder_path(&folder);
    let target_dir = if folder_clean.is_empty() { dir.clone() } else { dir.join(&folder_clean) };
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }

    let unchanged = old_exists
        && old_path.parent() == Some(target_dir.as_path())
        && old_path.file_stem().and_then(|s| s.to_str()) == Some(desired_base.as_str());

    let target_path = if unchanged {
        old_path.clone()
    } else {
        unique_path(&target_dir, &desired_base, if old_exists { Some(&old_path) } else { None })
    };

    if old_exists && target_path != old_path {
        fs::rename(&old_path, &target_path).or_else(|_| {
            fs::copy(&old_path, &target_path).map(|_| ()).and_then(|_| fs::remove_file(&old_path))
        }).map_err(|e| e.to_string())?;
    }

    fs::write(&target_path, serialize_note_file(&title, &body, &tags, pinned, &color))
        .map_err(|e| e.to_string())?;

    let rel = target_path.strip_prefix(&dir).unwrap_or(&target_path);
    let new_id = rel.with_extension("").to_string_lossy().replace('\\', "/");
    Ok(new_id)
}

#[tauri::command]
fn delete_note(id: String) -> Result<(), String> {
    let from = notes_dir().join(format!("{}.md", id));
    let to = trash_dir().join(format!("{}.md", id));
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if from.exists() {
        fs::rename(&from, &to).or_else(|_| {
            fs::copy(&from, &to).map(|_| ()).and_then(|_| fs::remove_file(&from))
        }).map_err(|e| e.to_string())?;
        // Bump the file's modified time to "now" so it accurately reflects
        // when it was deleted — used by the 14-day auto-purge.
        if let Ok(contents) = fs::read(&to) {
            let _ = fs::write(&to, contents);
        }
    }
    Ok(())
}

#[tauri::command]
fn restore_note(id: String) -> Result<(), String> {
    let from = trash_dir().join(format!("{}.md", id));
    let to = notes_dir().join(format!("{}.md", id));
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if from.exists() {
        fs::rename(&from, &to).or_else(|_| {
            fs::copy(&from, &to).map(|_| ()).and_then(|_| fs::remove_file(&from))
        }).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn purge_note(id: String) -> Result<(), String> {
    let path = trash_dir().join(format!("{}.md", id));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn reveal_folder() -> Result<(), String> {
    let dir = notes_dir();
    std::process::Command::new("open")
        .arg(dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_settings() -> String {
    fs::read_to_string(settings_path()).unwrap_or_else(|_| "{}".to_string())
}

#[tauri::command]
fn save_settings(json: String) -> Result<(), String> {
    fs::write(settings_path(), json).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
fn get_notes_dir() -> String {
    notes_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn set_notes_dir(path: String) -> Result<(), String> {
    let mut cfg = load_app_config();
    if path.trim().is_empty() {
        cfg.notes_dir = None;
    } else {
        let p = PathBuf::from(&path);
        fs::create_dir_all(&p).map_err(|e| e.to_string())?;
        cfg.notes_dir = Some(path);
    }
    save_app_config(&cfg)
}

#[tauri::command]
async fn pick_notes_folder(app: tauri::AppHandle) -> Option<String> {
    let folder = app.dialog().file().blocking_pick_folder();
    folder
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string())
}

/// Downloads an image from a URL (e.g. one dragged out of a web page) so
/// it can be saved locally the same way a pasted/dropped file is. Done in
/// Rust rather than the webview's own fetch() to sidestep CORS entirely -
/// most image hosts don't send the headers a browser-side fetch needs.
#[tauri::command]
async fn fetch_image_bytes(url: String) -> Result<Vec<u8>, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Not a web URL".to_string());
    }
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server responded with {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

#[tauri::command]
fn save_image(name: String, bytes: Vec<u8>) -> Result<String, String> {
    let dir = assets_dir();
    let ext = Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_string();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let filename = format!("image-{}.{}", stamp, ext);
    let path = dir.join(&filename);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(format!("assets/{}", filename))
}

fn debug_log(msg: &str) {
    use std::io::Write;
    let path = "/Users/johntaylor/Documents/MeglaNote Project/clip-debug.log";
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = writeln!(f, "[{}] {}", now, msg);
    }
}

fn handle_clip_url(app: &tauri::AppHandle, url_str: &str) {
    debug_log(&format!("handle_clip_url called with: {}", url_str));
    let parsed = match Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return,
    };
    if parsed.scheme() != "meglanote" {
        return;
    }
    let mut title = String::new();
    let mut page_url = String::new();
    let mut text = String::new();
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "title" => title = v.to_string(),
            "url" => page_url = v.to_string(),
            "text" => text = v.to_string(),
            _ => {}
        }
    }
    let payload = serde_json::json!({ "title": title, "url": page_url, "text": text });

    if let Some(state) = app.try_state::<ClipState>() {
        *state.0.lock().unwrap() = Some(payload.clone());
    }
    let _ = app.emit("clip-received", payload);

    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn get_pending_clip(state: tauri::State<ClipState>) -> Option<serde_json::Value> {
    state.0.lock().unwrap().take()
}

/// Lets the frontend write into the same debug log the deep-link plumbing
/// uses, so we can see exactly what the webview is doing without needing
/// to open browser dev tools.
#[tauri::command]
fn frontend_log(msg: String) {
    debug_log(&format!("[frontend] {}", msg));
}

/// Checks the update endpoint (see tauri.conf.json) for a newer release.
/// On success, stashes the `Update` handle in state so `install_update`
/// can use it without re-checking.
#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    debug_log("check_for_update: called");
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            debug_log(&format!("check_for_update: app.updater() failed: {}", e));
            return Err(e.to_string());
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            debug_log(&format!("check_for_update: update available -> {}", update.version));
            let info = UpdateInfo {
                version: update.version.clone(),
                notes: update.body.clone(),
            };
            if let Some(state) = app.try_state::<UpdateState>() {
                *state.0.lock().unwrap() = Some(update);
            }
            Ok(Some(info))
        }
        Ok(None) => {
            debug_log("check_for_update: no update available (already on latest)");
            Ok(None)
        }
        Err(e) => {
            debug_log(&format!("check_for_update: error -> {}", e));
            Err(e.to_string())
        }
    }
}

/// Downloads and installs the update found by the last `check_for_update`
/// call. Does not relaunch the app - the frontend tells the person to
/// quit and reopen once this succeeds.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let update = {
        let state = app
            .try_state::<UpdateState>()
            .ok_or("Updater not ready".to_string())?;
        let taken = state.0.lock().unwrap().take();
        taken
    };
    let update = update.ok_or("No update ready - check for updates first.".to_string())?;
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            debug_log("=== app setup() ran (process started) ===");
            app.manage(ClipState(Mutex::new(None)));
            app.manage(UpdateState(Mutex::new(None)));
            purge_old_trash();

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                let urls = event.urls();
                debug_log(&format!("on_open_url fired, {} url(s)", urls.len()));
                for u in urls {
                    handle_clip_url(&handle, u.as_str());
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                let handle2 = app.handle().clone();
                for u in urls {
                    handle_clip_url(&handle2, u.as_str());
                }
            }

            let app_menu = SubmenuBuilder::new(app, "MeglaNote")
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .text("new_note", "New Note")
                .separator()
                .text("show_in_finder", "Show Notes in Finder")
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let window_menu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .close_window()
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu, &file_menu, &edit_menu, &window_menu])
                .build()?;
            app.set_menu(menu)?;

            if let Some(main_window) = app.get_webview_window("main") {
                let win_to_hide = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Closing the window shouldn't kill the app - a clip that
                        // arrives afterwards needs a live "main" window to show
                        // into. Hide instead of destroying it; use MeglaNote > Quit
                        // (or Cmd+Q) to actually exit.
                        api.prevent_close();
                        let _ = win_to_hide.hide();
                    }
                });
            }

            let handle3 = app.handle().clone();
            app.on_menu_event(move |_app_handle, event| match event.id().0.as_str() {
                "new_note" => {
                    let _ = handle3.emit("menu-new-note", ());
                }
                "show_in_finder" => {
                    let _ = std::process::Command::new("open").arg(notes_dir()).spawn();
                }
                _ => {}
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_notes,
            list_trash,
            list_folders,
            create_folder,
            rename_folder,
            delete_folder,
            save_note,
            delete_note,
            restore_note,
            purge_note,
            reveal_folder,
            load_settings,
            save_settings,
            get_app_version,
            get_notes_dir,
            set_notes_dir,
            pick_notes_folder,
            save_image,
            fetch_image_bytes,
            get_pending_clip,
            check_for_update,
            install_update,
            frontend_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
