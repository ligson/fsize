use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::thread;
use tauri::{command, AppHandle, Emitter};

#[derive(Serialize, Deserialize, Debug)]
struct Node {
    name: String,
    path: String, // 增加 path 字段
    size: u64,
    is_dir: bool,
    children: HashMap<String, Node>,
}

#[command]
pub fn file_size(path: String, app_handle: AppHandle) {
    thread::spawn(move || {
        let path = Path::new(&path).to_path_buf();

        let mut root = Node {
            name: path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .to_string(),
            path: path.to_string_lossy().to_string(),
            size: 0,
            is_dir: path.is_dir(),
            children: HashMap::new(),
        };

        let result = match traverse_directory(&path, &mut root) {
            Ok(_) => serde_json::to_string(&root).map_err(|e| e.to_string()),
            Err(e) => Err(format!("Error traversing directory: {}", e)),
        };

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
                        "file_size_result",
                        Some(format!("{{\"error\": \"{}\"}}", e)),
                    )
                    .expect("Failed to emit error");
            }
        }
    });
}

fn traverse_directory(path: &Path, node: &mut Node) -> std::io::Result<()> {
    if path.is_file() {
        node.size = path.metadata()?.len();
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let mut child_node = Node {
                name: path
                    .file_name()
                    .unwrap_or_else(|| path.as_os_str())
                    .to_string_lossy()
                    .to_string(),
                path: path.to_string_lossy().to_string(), // 设置 path 字段
                size: 0,
                is_dir: path.is_dir(),
                children: HashMap::new(),
            };

            traverse_directory(&path, &mut child_node)?;
            node.size += child_node.size;
            node.children.insert(child_node.name.clone(), child_node);
        }
    }
    Ok(())
}
