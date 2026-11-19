// Directory scans use a shared, bounded worker pool.
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Instant;
use tauri::{command, AppHandle, Emitter};

static SCAN_POOL: OnceLock<Option<rayon::ThreadPool>> = OnceLock::new();
static SCAN_CACHE: OnceLock<RwLock<Option<ScanCache>>> = OnceLock::new();

fn scan_cache() -> &'static RwLock<Option<ScanCache>> {
    SCAN_CACHE.get_or_init(|| RwLock::new(None))
}

fn scan_pool() -> Option<&'static rayon::ThreadPool> {
    SCAN_POOL
        .get_or_init(|| {
            let worker_count = thread::available_parallelism()
                .map(|count| count.get().saturating_sub(1).clamp(1, 4))
                .unwrap_or(4);
            rayon::ThreadPoolBuilder::new()
                .num_threads(worker_count)
                .build()
                .ok()
        })
        .as_ref()
}

#[derive(Debug)]
struct Node {
    name: String,
    path: String, // 增加 path 字段
    size: u64,
    is_dir: bool,
    children: Vec<Arc<Node>>,
    readable: bool,
    complete: bool,
}

struct ScanCache {
    root: Arc<Node>,
    directories: HashMap<String, Arc<Node>>,
}

#[derive(Serialize)]
struct TreeEntry {
    name: String,
    path: String,
    size: u64,
    is_dir: bool,
    readable: bool,
    complete: bool,
    has_children: bool,
}

impl From<&Node> for TreeEntry {
    fn from(node: &Node) -> Self {
        Self {
            name: node.name.clone(),
            path: node.path.clone(),
            size: node.size,
            is_dir: node.is_dir,
            readable: node.readable,
            complete: node.complete,
            has_children: !node.children.is_empty(),
        }
    }
}

#[derive(Serialize)]
struct ScanSummary {
    root: TreeEntry,
    item_count: usize,
    incomplete_count: usize,
}

#[derive(Serialize)]
pub struct ChildrenPage {
    entries: Vec<TreeEntry>,
    offset: usize,
    total: usize,
    has_more: bool,
}

#[derive(Serialize)]
pub struct SearchResult {
    name: String,
    path: String,
    size: u64,
    readable: bool,
    complete: bool,
}

#[derive(Serialize, Clone)]
struct ScanProgressEvent {
    path: String,
    entries_scanned: u64,
}

struct ScanProgress {
    app_handle: AppHandle,
    active_directory: Mutex<String>,
    entries_scanned: AtomicU64,
    last_emit_ms: AtomicU64,
    started: Instant,
}

impl ScanProgress {
    fn new(app_handle: AppHandle, root: &Path) -> Self {
        Self {
            app_handle,
            active_directory: Mutex::new(root.to_string_lossy().to_string()),
            entries_scanned: AtomicU64::new(0),
            last_emit_ms: AtomicU64::new(u64::MAX),
            started: Instant::now(),
        }
    }

    fn set_directory(&self, path: &Path) {
        if let Ok(mut active_directory) = self.active_directory.lock() {
            *active_directory = path.to_string_lossy().to_string();
        }
        self.emit_if_due(false);
    }

    fn entry_scanned(&self) {
        self.entries_scanned.fetch_add(1, Ordering::Relaxed);
        self.emit_if_due(false);
    }

    fn emit_if_due(&self, force: bool) {
        let elapsed_ms = self.started.elapsed().as_millis() as u64;
        let previous = self.last_emit_ms.load(Ordering::Relaxed);
        if !force && previous != u64::MAX && elapsed_ms.saturating_sub(previous) < 250 {
            return;
        }
        if self
            .last_emit_ms
            .compare_exchange(previous, elapsed_ms, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let path = self
            .active_directory
            .lock()
            .map(|path| path.clone())
            .unwrap_or_default();
        let _ = self.app_handle.emit(
            "file_size_progress",
            ScanProgressEvent {
                path,
                entries_scanned: self.entries_scanned.load(Ordering::Relaxed),
            },
        );
    }
}

#[command]
pub fn file_size(path: String, app_handle: AppHandle) {
    if let Ok(mut cache) = scan_cache().write() {
        *cache = None;
    }
    thread::spawn(move || {
        let path = Path::new(&path).to_path_buf();
        let file_name = path
            .file_name()
            .unwrap_or_else(|| path.as_os_str())
            .to_string_lossy()
            .to_string();
        println!("{}", file_name);
        let is_dir = path.is_dir();
        let root_metadata = if is_dir { None } else { path.metadata().ok() };
        let mut root = Node {
            name: file_name,
            path: path.to_string_lossy().to_string(),
            size: root_metadata
                .as_ref()
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            is_dir,
            children: Vec::new(),
            readable: is_dir || root_metadata.is_some(),
            complete: is_dir || root_metadata.is_some(),
        };
        let progress = ScanProgress::new(app_handle.clone(), &path);
        progress.emit_if_due(true);

        match scan_pool() {
            Some(pool) => pool.install(|| traverse_directory(&path, &mut root, &progress)),
            None => traverse_directory(&path, &mut root, &progress),
        }
        let root = Arc::new(root);
        let mut directories = HashMap::new();
        let mut pending = vec![Arc::clone(&root)];
        let mut item_count = 0;
        let mut incomplete_count = 0;
        while let Some(node) = pending.pop() {
            item_count += 1;
            incomplete_count += usize::from(!node.complete);
            if node.is_dir {
                directories.insert(node.path.clone(), Arc::clone(&node));
            }
            pending.extend(node.children.iter().cloned());
        }

        let summary = ScanSummary {
            root: TreeEntry::from(root.as_ref()),
            item_count,
            incomplete_count,
        };
        if let Ok(mut cache) = scan_cache().write() {
            *cache = Some(ScanCache { root, directories });
        }
        let result = serde_json::to_string(&summary).map_err(|e| e.to_string());

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

#[command]
pub fn file_size_children(
    path: String,
    offset: usize,
    limit: usize,
) -> Result<ChildrenPage, String> {
    let directory = {
        let cache = scan_cache()
            .read()
            .map_err(|_| "扫描结果暂时不可用".to_string())?;
        cache
            .as_ref()
            .and_then(|cache| cache.directories.get(&path))
            .cloned()
            .ok_or_else(|| "目录扫描结果已失效，请重新扫描".to_string())?
    };

    let total = directory.children.len();
    let offset = offset.min(total);
    let page_size = limit.clamp(1, 1000);
    let end = offset.saturating_add(page_size).min(total);
    let entries = directory.children[offset..end]
        .iter()
        .map(|node| TreeEntry::from(node.as_ref()))
        .collect();

    Ok(ChildrenPage {
        entries,
        offset,
        total,
        has_more: end < total,
    })
}

#[command]
pub async fn file_search(query: String, limit: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let root = {
        let cache = scan_cache()
            .read()
            .map_err(|_| "扫描结果暂时不可用".to_string())?;
        cache
            .as_ref()
            .map(|cache| Arc::clone(&cache.root))
            .ok_or_else(|| "请先完成目录扫描".to_string())?
    };
    let query = query.trim().to_lowercase();
    let limit = limit.unwrap_or(1000).clamp(1, 1000);

    tauri::async_runtime::spawn_blocking(move || {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut pending = vec![root.as_ref()];
        while let Some(node) = pending.pop() {
            if !node.is_dir
                && (node.name.to_lowercase().contains(&query)
                    || node.path.to_lowercase().contains(&query))
            {
                results.push(SearchResult {
                    name: node.name.clone(),
                    path: node.path.clone(),
                    size: node.size,
                    readable: node.readable,
                    complete: node.complete,
                });
                if results.len() >= limit {
                    break;
                }
            }
            pending.extend(node.children.iter().map(Arc::as_ref));
        }
        results
    })
    .await
    .map_err(|error| error.to_string())
}

fn traverse_directory(path: &Path, node: &mut Node, progress: &ScanProgress) {
    if !node.is_dir {
        return;
    }

    progress.set_directory(path);

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            node.readable = false;
            node.complete = false;
            return;
        }
    };
    let mut temp_children = Vec::new();
    let mut pending_children: Vec<(Arc<Node>, fs::DirEntry)> = Vec::new();
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
        progress.entry_scanned();
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                complete = false;
                temp_children.push(Arc::new(Node {
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
                }));
                continue;
            }
        };
        if file_type.is_symlink() {
            continue;
        }

        let child_node = Arc::new(Node {
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
        });

        pending_children.push((child_node, entry));
    }

    if pending_children.len() >= 8 {
        pending_children.par_iter_mut().for_each(|(child, entry)| {
            process_pending_entry(child, entry, progress);
        });
    } else {
        for (child, entry) in &mut pending_children {
            process_pending_entry(child, entry, progress);
        }
    }
    temp_children.extend(pending_children.into_iter().map(|(child, _)| child));

    for child in &temp_children {
        complete &= child.complete;
        node.size = node.size.saturating_add(child.size);
    }

    temp_children.sort_by(|a, b| b.size.cmp(&a.size));
    node.children = temp_children;
    node.complete = complete;
}

fn process_pending_entry(child: &mut Arc<Node>, entry: &fs::DirEntry, progress: &ScanProgress) {
    let child = Arc::get_mut(child).expect("unshared scan node");
    if child.is_dir {
        let child_path = entry.path();
        traverse_directory(&child_path, child, progress);
        return;
    }

    match entry.metadata() {
        Ok(metadata) => child.size = metadata.len(),
        Err(_) => {
            child.readable = false;
            child.complete = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{file_size_children, scan_cache, Node, ScanCache};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn directory_children_are_returned_in_pages() {
        let children = (0..3)
            .map(|index| {
                Arc::new(Node {
                    name: format!("file-{index}"),
                    path: format!("root/file-{index}"),
                    size: index,
                    is_dir: false,
                    children: Vec::new(),
                    readable: true,
                    complete: true,
                })
            })
            .collect::<Vec<_>>();
        let root = Arc::new(Node {
            name: "root".to_string(),
            path: "root".to_string(),
            size: 3,
            is_dir: true,
            children,
            readable: true,
            complete: true,
        });
        let mut directories = HashMap::new();
        directories.insert("root".to_string(), Arc::clone(&root));
        *scan_cache().write().unwrap() = Some(ScanCache { root, directories });

        let page = file_size_children("root".to_string(), 1, 1).unwrap();

        assert_eq!(page.total, 3);
        assert_eq!(page.offset, 1);
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].name, "file-1");
        assert!(page.has_more);
    }
}
