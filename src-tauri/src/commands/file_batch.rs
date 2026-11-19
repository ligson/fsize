use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct BatchFailure {
    path: String,
    error: String,
}

#[derive(Serialize)]
pub struct BatchResult {
    succeeded: Vec<String>,
    failed: Vec<BatchFailure>,
}

fn describe_error(action: &str, path: &Path, error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::PermissionDenied {
        format!("没有权限{}：{}", action, path.display())
    } else {
        format!("{}失败：{} ({})", action, path.display(), error)
    }
}

fn remove_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[tauri::command]
pub async fn file_delete_many(paths: Vec<String>) -> Result<BatchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut paths = paths.into_iter().map(PathBuf::from).collect::<Vec<_>>();
        paths.sort_by_key(|path| path.components().count());
        let mut result = BatchResult {
            succeeded: Vec::new(),
            failed: Vec::new(),
        };
        let mut removed_directories = Vec::<PathBuf>::new();

        for path in paths {
            if removed_directories
                .iter()
                .any(|parent| path.starts_with(parent))
            {
                continue;
            }
            let is_directory = fs::symlink_metadata(&path)
                .map(|metadata| metadata.file_type().is_dir())
                .unwrap_or(false);
            match remove_path(&path) {
                Ok(()) => {
                    if is_directory {
                        removed_directories.push(path.clone());
                    }
                    result.succeeded.push(path.to_string_lossy().into_owned());
                }
                Err(error) => result.failed.push(BatchFailure {
                    path: path.to_string_lossy().into_owned(),
                    error: describe_error("删除", &path, &error),
                }),
            }
        }

        result
    })
    .await
    .map_err(|error| error.to_string())
}

fn copy_entry(source: &Path, target: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        let link_target = fs::read_link(source)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(link_target, target)?;
        #[cfg(windows)]
        {
            if fs::metadata(source)
                .map(|target| target.is_dir())
                .unwrap_or(false)
            {
                std::os::windows::fs::symlink_dir(link_target, target)?;
            } else {
                std::os::windows::fs::symlink_file(link_target, target)?;
            }
        }
        return Ok(());
    }

    if file_type.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &target.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(source, target).map(|_| ())
    }
}

fn is_cross_device_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(18)
    }
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(17)
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn move_one(source: &Path, destination_dir: &Path) -> io::Result<PathBuf> {
    let source_metadata = fs::symlink_metadata(source)?;
    let name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "不能移动文件系统根目录"))?;
    let target = destination_dir.join(name);

    if target == source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "源路径与目标路径相同",
        ));
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "目标目录中已存在同名项目",
        ));
    }
    if source_metadata.is_dir() {
        let canonical_source = fs::canonicalize(source)?;
        let canonical_destination = fs::canonicalize(destination_dir)?;
        if canonical_destination.starts_with(&canonical_source) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "不能将目录移动到自身或其子目录中",
            ));
        }
    }

    match fs::rename(source, &target) {
        Ok(()) => Ok(target),
        Err(error) if is_cross_device_error(&error) => {
            if let Err(copy_error) = copy_entry(source, &target) {
                let _ = remove_path(&target);
                return Err(copy_error);
            }
            remove_path(source)?;
            Ok(target)
        }
        Err(error) => Err(error),
    }
}

#[tauri::command]
pub async fn file_move_many(
    paths: Vec<String>,
    destination: String,
) -> Result<BatchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let destination = PathBuf::from(destination);
        let mut result = BatchResult {
            succeeded: Vec::new(),
            failed: Vec::new(),
        };

        if !destination.is_dir() {
            result
                .failed
                .extend(paths.into_iter().map(|path| BatchFailure {
                    path,
                    error: format!("目标目录不存在或不可访问：{}", destination.display()),
                }));
            return result;
        }

        for source in paths.into_iter().map(PathBuf::from) {
            match move_one(&source, &destination) {
                Ok(target) => result.succeeded.push(target.to_string_lossy().into_owned()),
                Err(error) => result.failed.push(BatchFailure {
                    path: source.to_string_lossy().into_owned(),
                    error: describe_error("移动", &source, &error),
                }),
            }
        }

        result
    })
    .await
    .map_err(|error| error.to_string())
}
