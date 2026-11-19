#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let command = "open";

    #[cfg(target_os = "windows")]
    let command = "explorer";

    #[cfg(target_os = "linux")]
    let command = "xdg-open";

    std::process::Command::new(command)
        .arg(&path)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!("没有权限打开路径：{}", path)
            } else {
                format!("打开路径失败：{} ({})", path, e)
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    let target = std::path::Path::new(&path);

    #[cfg(target_os = "windows")]
    let result = if target.is_dir() {
        std::process::Command::new("explorer").arg(target).spawn()
    } else {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", target.display()))
            .spawn()
    };

    #[cfg(target_os = "macos")]
    let result = if target.is_dir() {
        std::process::Command::new("open").arg(target).spawn()
    } else {
        std::process::Command::new("open")
            .arg("-R")
            .arg(target)
            .spawn()
    };

    #[cfg(target_os = "linux")]
    let result = {
        let directory = if target.is_dir() {
            target
        } else {
            target.parent().unwrap_or(target)
        };
        std::process::Command::new("xdg-open")
            .arg(directory)
            .spawn()
    };

    result.map(|_| ()).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            format!("没有权限在文件管理器中定位：{}", path)
        } else {
            format!("在文件管理器中定位失败：{} ({})", path, error)
        }
    })
}
