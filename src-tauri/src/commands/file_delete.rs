use std::path::Path;
use std::{fs, thread};
use tauri::{command, AppHandle, Emitter, Result};

fn describe_error(action: &str, path: &Path, error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        format!("没有权限{}：{}", action, path.display())
    } else {
        format!("{}失败：{} ({})", action, path.display(), error)
    }
}

#[command]
pub fn file_delete(path: String, app_handle: AppHandle) -> Result<()> {
    thread::spawn(move || {
        let path = Path::new(&path);

        // 获取文件或目录的元数据
        let result = fs::metadata(path);

        match result {
            Ok(metadata) => {
                if metadata.is_file() {
                    // 如果是文件，删除文件
                    if let Err(e) = fs::remove_file(path) {
                        app_handle
                            .emit("file_deleted_error", describe_error("删除", path, &e))
                            .unwrap();
                        return;
                    }
                } else if metadata.is_dir() {
                    // 如果是目录，删除目录及其内容
                    if let Err(e) = fs::remove_dir_all(path) {
                        app_handle
                            .emit("file_deleted_error", describe_error("删除", path, &e))
                            .unwrap();
                        return;
                    }
                }
                // 发送删除成功事件
                app_handle
                    .emit("file_deleted", "Successfully deleted")
                    .unwrap();
            }
            Err(e) => {
                app_handle
                    .emit("file_deleted_error", describe_error("访问", path, &e))
                    .unwrap();
            }
        }
    });

    Ok(())
}
