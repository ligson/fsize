// 在文件顶部添加
use serde::{Deserialize, Serialize};
use std::backtrace::Backtrace;
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
    children: Vec<Node>,
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
        };

        let result = match traverse_directory(&path, &mut root) {
            Ok(_) => serde_json::to_string(&root).map_err(|e| e.to_string()),
            Err(e) => {
                let bt = Backtrace::capture();
                println!("stack:\n{}", bt);
                Err(format!(
                    "Error traversing directory: {},err:{}",
                    path.to_string_lossy().to_string(),
                    e
                ))
            }
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
                        "file_size_result_error",
                        Some(format!("{{\"error\": \"{}\"}}", e)),
                    )
                    .expect("Failed to emit error");
            }
        }
    });
}
fn traverse_directory(path: &Path, node: &mut Node) -> std::io::Result<()> {
    if !path.is_dir() {
        node.size = path.metadata()?.len();
        return Ok(());
    }
    let result = fs::read_dir(path);
    if result.is_err() {
        return Err(result.unwrap_err());
    }
    let entries: Vec<_> = result?.collect();
    let mut temp_children = Vec::new();

    // 第一阶段：收集子节点并计算 size
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }

        // 创建基础节点
        let mut child_node = Node {
            name: path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .to_string(),
            path: path.to_string_lossy().to_string(),
            size: 0,
            is_dir: path.is_dir(),
            children: Vec::new(),
        };

        // 递归遍历或获取文件 size
        if child_node.is_dir {
            traverse_directory(&path, &mut child_node)?;
        } else {
            child_node.size = path.metadata()?.len();
        }

        node.size += child_node.size;
        temp_children.push(child_node);
    }

    // 第二阶段：排序当前层级的子节点
    temp_children.sort_by(|a, b| b.size.cmp(&a.size));

    // 第三阶段：存入父节点
    node.children = temp_children;

    Ok(())
}
