//! Read-only active-folder snapshot for file-manager integrations such as Listary.
//! Queries never call into GPUI or activate/navigate the FileFlow window.
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

pub const QUERY: &str = "fileflow-query:current-directory:v1";

#[derive(Clone, Default)]
struct Snapshot {
    window_handle: usize,
    path: Option<PathBuf>,
}

static CURRENT: OnceLock<RwLock<Snapshot>> = OnceLock::new();

pub fn active_directory(left: &Path, right: &Path, split: bool, side: &str) -> Option<PathBuf> {
    let path = if split && side == "right" { right } else { left };
    (!crate::is_virtual_path(path)).then(|| crate::normalize_shell_path(path))
}

pub fn publish(window_handle: usize, path: Option<PathBuf>) {
    let current = CURRENT.get_or_init(|| RwLock::new(Snapshot::default()));
    if let Ok(mut snapshot) = current.write()
        && (snapshot.window_handle != window_handle || snapshot.path != path) {
        *snapshot = Snapshot { window_handle, path };
    }
}

fn encode(snapshot: &Snapshot) -> String {
    serde_json::json!({
        "version": 1,
        "window_handle": snapshot.window_handle,
        "path": snapshot.path.as_ref().map(|path| path.to_string_lossy()),
    }).to_string()
}

pub fn response(request: &str) -> Option<String> {
    if request.trim() != QUERY {
        return request.trim().starts_with("fileflow-query:")
            .then(|| serde_json::json!({"version": 1, "window_handle": 0, "path": null, "error": "unsupported query"}).to_string());
    }
    let snapshot = CURRENT.get_or_init(|| RwLock::new(Snapshot::default()))
        .read().map(|value| value.clone()).unwrap_or_default();
    Some(encode(&snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_folder_follows_pane_and_excludes_virtual_views() {
        let left = Path::new(r"C:\中文 文件夹");
        let right = Path::new("E:/图片 # 目录");
        assert_eq!(active_directory(left, right, false, "right"), Some(left.to_path_buf()));
        assert_eq!(active_directory(left, right, true, "left"), Some(left.to_path_buf()));
        assert_eq!(active_directory(left, right, true, "right"), Some(PathBuf::from(r"E:\图片 # 目录")));
        let virtual_path = PathBuf::from(crate::THIS_PC_PATH);
        assert!(active_directory(&virtual_path, right, false, "left").is_none());
        assert!(active_directory(left, &virtual_path, true, "right").is_none());
    }

    #[test]
    fn query_encodes_unicode_path_and_window_without_navigating() {
        let snapshot = Snapshot { window_handle: 123456, path: Some(PathBuf::from(r"E:\中文\图片 # 目录")) };
        let json: serde_json::Value = serde_json::from_str(&encode(&snapshot)).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["window_handle"], 123456);
        assert_eq!(json["path"], r"E:\中文\图片 # 目录");
        assert!(response(r"C:\ordinary-open-command").is_none());
        assert!(response("fileflow-menu:paste:right").is_none());
        let unsupported: serde_json::Value = serde_json::from_str(&response("fileflow-query:unknown").unwrap()).unwrap();
        assert!(unsupported["path"].is_null());
        let empty: serde_json::Value = serde_json::from_str(&encode(&Snapshot::default())).unwrap();
        assert!(empty["path"].is_null());
    }
}
