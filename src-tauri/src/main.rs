#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};
use rusqlite::{params, params_from_iter, Connection};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use walkdir::WalkDir;

struct AppState {
    db: Arc<Mutex<Connection>>,
    indexing: Arc<AtomicBool>,
    count: Arc<AtomicU64>,
}

#[derive(Serialize)]
struct Item {
    name: String,
    path: String,
    is_dir: bool,
}

#[derive(Serialize)]
struct Status {
    indexing: bool,
    count: u64,
}

type Row = (String, String, String, String, i64);

fn row_of(p: &Path) -> Option<Row> {
    let name = p.file_name()?.to_string_lossy().to_string();
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    Some((p.to_string_lossy().to_string(), name.clone(), name.to_lowercase(), ext, p.is_dir() as i64))
}

fn insert_rows(conn: &mut Connection, rows: &[Row]) {
    if let Ok(tx) = conn.transaction() {
        {
            let mut st = tx
                .prepare_cached("INSERT OR REPLACE INTO files(path,name,name_lower,ext,is_dir) VALUES(?1,?2,?3,?4,?5)")
                .unwrap();
            for r in rows {
                let _ = st.execute(params![r.0, r.1, r.2, r.3, r.4]);
            }
        }
        let _ = tx.commit();
    }
}

fn roots() -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .map(|c| PathBuf::from(format!("{}:\\", c as char)))
        .filter(|p| p.exists())
        .collect()
}

fn skip(name: &str) -> bool {
    matches!(name, "$Recycle.Bin" | "System Volume Information" | "$WinREAgent" | "node_modules" | ".git")
}

fn index_all(db: Arc<Mutex<Connection>>, indexing: Arc<AtomicBool>, count: Arc<AtomicU64>) {
    if indexing.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || {
        let mut batch: Vec<Row> = Vec::with_capacity(5000);
        for root in roots() {
            let it = WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !skip(&e.file_name().to_string_lossy()));
            for e in it.filter_map(Result::ok) {
                if let Some(r) = row_of(e.path()) {
                    batch.push(r);
                }
                if batch.len() >= 5000 {
                    insert_rows(&mut db.lock().unwrap(), &batch);
                    batch.clear();
                }
            }
        }
        if !batch.is_empty() {
            insert_rows(&mut db.lock().unwrap(), &batch);
        }
        // মুছে ফেলা ফাইলের পুরনো এন্ট্রি পরিষ্কার (হালকা চেক)
        {
            let c = db.lock().unwrap();
            let n: u64 = c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
            count.store(n, Ordering::SeqCst);
        }
        indexing.store(false, Ordering::SeqCst);
    });
}

fn start_watcher(db: Arc<Mutex<Connection>>, count: Arc<AtomicU64>) {
    thread::spawn(move || {
        let db2 = db.clone();
        let mut w = match recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            if matches!(ev.kind, EventKind::Access(_)) {
                return;
            }
            let mut c = db2.lock().unwrap();
            for p in ev.paths {
                let s = p.to_string_lossy().to_string();
                if s.contains("$Recycle.Bin") || s.contains("\\.git\\") || s.contains("\\node_modules\\") {
                    continue;
                }
                if p.exists() {
                    if let Some(r) = row_of(&p) {
                        insert_rows(&mut c, &[r]);
                    }
                } else {
                    let like = format!("{}\\%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
                    let _ = c.execute("DELETE FROM files WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\'", params![s, like]);
                }
            }
            let n: u64 = c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
            count.store(n, Ordering::SeqCst);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        for r in roots() {
            let _ = w.watch(&r, RecursiveMode::Recursive);
        }
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    });
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[tauri::command]
fn search(state: State<AppState>, query: String, ext: Option<String>, limit: Option<usize>) -> Vec<Item> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let limit = limit.unwrap_or(100);
    let conn = state.db.lock().unwrap();
    let mut out: Vec<Item> = vec![];
    let mut seen: HashSet<String> = HashSet::new();

    // ধাপ ১: substring (শব্দ ক্রমে), ধাপ ২: fuzzy (অক্ষর ক্রমে)
    let substring = format!("%{}%", q.split_whitespace().map(esc).collect::<Vec<_>>().join("%"));
    let fuzzy = format!("%{}%", q.chars().filter(|c| !c.is_whitespace()).map(|c| esc(&c.to_string())).collect::<Vec<_>>().join("%"));

    for pat in [substring, fuzzy] {
        if out.len() >= limit {
            break;
        }
        let mut sql = String::from("SELECT name,path,is_dir FROM files WHERE name_lower LIKE ?1 ESCAPE '\\'");
        let mut args: Vec<String> = vec![pat];
        if let Some(e) = ext.as_ref().filter(|e| !e.is_empty()) {
            sql.push_str(" AND ext = ?2");
            args.push(e.to_lowercase().trim_start_matches('.').to_string());
        }
        sql.push_str(" ORDER BY is_dir DESC, length(name) ASC LIMIT ");
        sql.push_str(&(limit * 2).to_string());
        let mut st = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let rows = st.query_map(params_from_iter(args.iter()), |r| {
            Ok(Item { name: r.get(0)?, path: r.get(1)?, is_dir: r.get::<_, i64>(2)? != 0 })
        });
        if let Ok(rows) = rows {
            for it in rows.filter_map(Result::ok) {
                if out.len() >= limit {
                    break;
                }
                if seen.insert(it.path.clone()) {
                    out.push(it);
                }
            }
        }
    }
    out
}

#[tauri::command]
fn open_path(path: String) {
    let _ = Command::new("explorer").arg(path).spawn();
}

#[tauri::command]
fn reveal_path(path: String) {
    let _ = Command::new("explorer").arg(format!("/select,{}", path)).spawn();
}

#[tauri::command]
fn status(state: State<AppState>) -> Status {
    Status { indexing: state.indexing.load(Ordering::SeqCst), count: state.count.load(Ordering::SeqCst) }
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

fn toggle(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            let _ = app.emit("focus-search", ());
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _sc, ev| {
                    if ev.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Connection::open(dir.join("index.db"))?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=OFF;
                 CREATE TABLE IF NOT EXISTS files(
                   path TEXT PRIMARY KEY, name TEXT, name_lower TEXT, ext TEXT, is_dir INTEGER);
                 CREATE INDEX IF NOT EXISTS idx_ext ON files(ext);",
            )?;
            let existing: u64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
            let db = Arc::new(Mutex::new(conn));
            let indexing = Arc::new(AtomicBool::new(false));
            let count = Arc::new(AtomicU64::new(existing));

            index_all(db.clone(), indexing.clone(), count.clone());
            start_watcher(db.clone(), count.clone());
            app.manage(AppState { db, indexing, count });

            app.global_shortcut().register(Shortcut::new(Some(Modifiers::CONTROL), Code::Space))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search, open_path, reveal_path, status, hide_window])
        .run(tauri::generate_context!())
        .expect("error while running FastFind");
}
