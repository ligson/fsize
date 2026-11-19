// Directory scans use a shared, bounded worker pool.
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use tauri::{command, AppHandle, Emitter};

static SCAN_POOL: OnceLock<Option<rayon::ThreadPool>> = OnceLock::new();

fn scan_pool() -> Option<&'static rayon::ThreadPool> {
    SCAN_POOL
        .get_or_init(|| {
            let worker_count = thread::available_parallelism()
                .map(|count| count.get().clamp(2, 8))
                .unwrap_or(4);
            rayon::ThreadPoolBuilder::new()
                .num_threads(worker_count)
                .build()
                .ok()
        })
        .as_ref()
}

#[derive(Serialize, Debug)]
struct Node {
    name: String,
    path: String, // 增加 path 字段
    size: u64,
    is_dir: bool,
    children: Vec<Node>,
    readable: bool,
    complete: bool,
}

#[command]
pub fn file_size(path: String, app_handle: AppHandle) {
    thread::spawn(move || {
        let path = Path::new(&path).to_path_buf();
        let file_name = path
            .file_name()
            .unwrap_or_else(|| path.as_os_str())
            .to_string_lossy()
            .to_string();
        println!("{}", file_name);
        let mut root = Node {
            name: file_name,
            path: path.to_string_lossy().to_string(),
            size: 0,
            is_dir: path.is_dir(),
            children: Vec::new(),
            readable: true,
            complete: true,
        };

        match scan_pool() {
            Some(pool) => pool.install(|| traverse_directory(&path, &mut root)),
            None => traverse_directory(&path, &mut root),
        }
        let result = serde_json::to_string(&root).map_err(|e| e.to_string());

        match result {
            Ok(json_result) => {
                // 主动 emit 事件，把结果发给前端
                app_handle
                    .emit("file_size_result", Some(json_result))
                    .expect("Failed to emit event");
            }
            Err(e) => {
                app_handle
                    .emit(
                        "file_size_result_error",
                        Some(format!("{{\"error\": \"{}\"}}", e)),
                    )
                    .expect("Failed to emit error");
            }
        }
    });
}
fn traverse_directory(path: &Path, node: &mut Node) {
    if !node.is_dir {
        match path.metadata() {
            Ok(metadata) => node.size = metadata.len(),
            Err(_) => {
                node.readable = false;
                node.complete = false;
            }
        }
        return;
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            node.readable = false;
            node.complete = false;
            return;
        }
    };
    let mut temp_children = Vec::new();
    let mut complete = true;

    // Keep entries whose metadata or contents cannot be read so users can still act on them.
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                complete = false;
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                complete = false;
                temp_children.push(Node {
                    name: path
                        .file_name()
                        .unwrap_or_else(|| path.as_os_str())
                        .to_string_lossy()
                        .to_string(),
                    path: path.to_string_lossy().to_string(),
                    size: 0,
                    is_dir: false,
                    children: Vec::new(),
                    readable: false,
                    complete: false,
                });
                continue;
            }
        };
        if file_type.is_symlink() {
            continue;
        }

        let child_node = Node {
            name: path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .to_string(),
            path: path.to_string_lossy().to_string(),
            size: 0,
            is_dir: file_type.is_dir(),
            children: Vec::new(),
            readable: true,
            complete: true,
        };

        temp_children.push(child_node);
    }

    if temp_children.len() >= 8 {
        temp_children.par_iter_mut().for_each(|child| {
            let child_path = PathBuf::from(&child.path);
            traverse_directory(&child_path, child);
        });
    } else {
        for child in &mut temp_children {
            let child_path = PathBuf::from(&child.path);
            traverse_directory(&child_path, child);
        }
    }

    for child in &temp_children {
        complete &= child.complete;
        node.size = node.size.saturating_add(child.size);
    }

    temp_children.sort_by(|a, b| b.size.cmp(&a.size));
    node.children = temp_children;
    node.complete = complete;
}
