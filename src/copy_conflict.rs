//! Ask before cross-directory copy/move collisions. Keep same-directory duplication
//! and low-level Shell operations unchanged (undo must never open this prompt).
use crate::{
    ShellOperationKind, UndoEntry, cached_or_find_fileflow_hwnd, perform_shell_file_operation,
    transfer_undo::{
        MoveRecord, ShellOperationResult, UndoResult, undo_copied_items, undo_moved_items,
    },
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::HWND,
        UI::Controls::{
            TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION,
            TDF_POSITION_RELATIVE_TO_WINDOW, TDF_USE_COMMAND_LINKS, TaskDialogIndirect,
        },
    },
    core::PCWSTR,
};
use windows_sys::Win32::Foundation::ERROR_NOT_SAME_DEVICE;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Replace,
    Rename,
    Cancel,
}

fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
}

fn collisions(sources: &[PathBuf], target: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for source in sources {
        let Some(name) = source.file_name() else {
            return Err(io::Error::other("不能复制磁盘根目录"));
        };
        let dest = target.join(name);
        if same_path(source, &dest) {
            continue;
        } // Ctrl+C/V in the same folder remains a duplicate.
        match fs::symlink_metadata(&dest) {
            Ok(_) => {
                if !found.contains(&dest) {
                    found.push(dest);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e), // Don't assume inaccessible destinations are empty.
        }
    }
    Ok(found)
}

fn prompt(kind: ShellOperationKind, names: &[PathBuf]) -> Result<Choice, String> {
    let label = kind.progress_label();
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let title = wide("FileFlow — 同名文件");
    let heading = wide(&format!("目标位置已有 {} 个同名项目", names.len()));
    let listed = names
        .iter()
        .take(5)
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    let content = wide(&format!(
        "{listed}{}\n\n请选择本次{label}的处理方式：",
        if names.len() > 5 { "\n…" } else { "" }
    ));
    let replace = wide("覆盖\n替换目标中的同名项目，原内容保留供 Ctrl+Z 恢复；同名文件夹整体替换");
    let rename = wide(&format!(
        "重命名\n自动给{label}过来的项目生成新名称，保留目标中的原项目"
    ));
    let cancel = wide(&format!("取消\n取消本次{label}，不改变任何文件"));
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: 1001,
            pszButtonText: PCWSTR(replace.as_ptr()),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 1002,
            pszButtonText: PCWSTR(rename.as_ptr()),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 1003,
            pszButtonText: PCWSTR(cancel.as_ptr()),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: HWND(cached_or_find_fileflow_hwnd()),
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
            | TDF_POSITION_RELATIVE_TO_WINDOW
            | TDF_USE_COMMAND_LINKS,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(heading.as_ptr()),
        pszContent: PCWSTR(content.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 1003,
        ..Default::default()
    };
    let mut button = 0;
    unsafe { TaskDialogIndirect(&config, Some(&mut button), None, None) }
        .map_err(|e| format!("无法显示同名提示：{e}"))?;
    Ok(match button {
        1001 => Choice::Replace,
        1002 => Choice::Rename,
        _ => Choice::Cancel,
    })
}

pub fn perform_user_transfer(
    kind: ShellOperationKind,
    sources: &[PathBuf],
    target: &Path,
) -> ShellOperationResult {
    transfer_with_choice(kind, sources, target, |names| prompt(kind, names))
}

fn empty_result(error: String) -> ShellOperationResult {
    ShellOperationResult {
        result: Err(error),
        moved: Vec::new(),
        copied: Vec::new(),
        replaced: Vec::new(),
    }
}

fn recovery_root() -> PathBuf {
    crate::config_path().with_file_name("overwrite-backups")
}

fn backup_dir() -> io::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = recovery_root();
    fs::create_dir_all(&root)?;
    let session = root.join(format!("{}-{nonce}", std::process::id()));
    fs::create_dir(&session)?;
    let files = session.join("files");
    if let Err(error) = fs::create_dir(&files) {
        let _ = fs::remove_dir(&session);
        return Err(error);
    }
    Ok(files)
}

fn record_backups(
    files: &Path,
    target: &Path,
    kind: &str,
    originals: &[PathBuf],
) -> io::Result<()> {
    use std::io::Write;
    let record = serde_json::json!({
        "format": "fileflow-overwrite-backups-v1",
        "operation": kind,
        "target_directory": target,
        "items": originals.iter().map(|original| serde_json::json!({
            "original_path": original,
            "backup_path": files.join(original.file_name().unwrap_or_default()),
        })).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec_pretty(&record)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(files.parent().expect("backup session").join("record.json"))?;
    file.write_all(&bytes)?;
    file.sync_all()
}

fn cleanup_empty_backup(files: &Path) {
    // Never remove backup contents. The metadata can go only after `files`
    // was successfully removed as an empty directory.
    if fs::remove_dir(files).is_err() {
        return;
    }
    let Some(session) = files.parent() else {
        return;
    };
    if files.file_name().is_some_and(|name| name == "files")
        && session
            .parent()
            .is_some_and(|parent| same_path(parent, &recovery_root()))
    {
        let _ = fs::remove_file(session.join("record.json"));
        let _ = fs::remove_dir(session);
    }
}

fn archive_item(original: &Path, backup: &Path) -> Result<(), String> {
    match fs::rename(original, backup) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE as i32) => {
            // A cross-volume directory move can partially remove the source on
            // cancellation. Finish a native copy first, then recycle the old
            // item; a failed copy leaves the original entirely intact.
            let result = perform_shell_file_operation(
                ShellOperationKind::Copy,
                &[original.to_path_buf()],
                backup.parent(),
                None,
            );
            if result.result.is_err() || !result.copied.iter().any(|item| same_path(item, backup)) {
                return Err(result
                    .result
                    .err()
                    .unwrap_or_else(|| "未能完整归档原内容".into()));
            }
            let removed = perform_shell_file_operation(
                ShellOperationKind::Delete,
                &[original.to_path_buf()],
                None,
                None,
            );
            if !original.exists() {
                Ok(())
            } else {
                Err(removed
                    .result
                    .err()
                    .unwrap_or_else(|| "原内容已归档，但未能移出工作目录".into()))
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

fn legacy_backup_name(name: &std::ffi::OsStr) -> bool {
    let text = name.to_string_lossy();
    let Some(suffix) = text.strip_prefix(".FileFlow-undo-") else {
        return false;
    };
    let Some((pid, nonce)) = suffix.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && pid.bytes().all(|c| c.is_ascii_digit())
        && nonce.len() >= 19
        && nonce.bytes().all(|c| c.is_ascii_digit())
}

/// Used only by the new version after the old process has exited: moving backups
/// while its in-memory undo entries were live would invalidate those entries.
pub fn migrate_legacy_backups(directory: &Path) -> io::Result<usize> {
    let mut migrated = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !legacy_backup_name(&entry.file_name()) || !entry.file_type()?.is_dir() {
            continue;
        }
        let legacy = entry.path();
        let saved = fs::read_dir(&legacy)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()?;
        if saved.is_empty() {
            let _ = fs::remove_dir(&legacy);
            continue;
        }
        let originals = saved
            .iter()
            .map(|item| directory.join(item.file_name().unwrap_or_default()))
            .collect::<Vec<_>>();
        let files = backup_dir()?;
        if let Err(error) = record_backups(&files, directory, "legacy", &originals) {
            cleanup_empty_backup(&files);
            return Err(error);
        }
        let mut problem = None;
        for item in saved {
            let backup = files.join(item.file_name().unwrap_or_default());
            if let Err(error) = archive_item(&item, &backup) {
                problem = Some(io::Error::other(error));
                break;
            }
            migrated += 1;
        }
        cleanup_empty_backup(&files);
        let _ = fs::remove_dir(&legacy); // Empty only, including partial failures.
        if let Some(error) = problem {
            return Err(error);
        }
    }
    Ok(migrated)
}

pub fn start_legacy_backup_migration() {
    let (settings, _) = crate::read_config_object(&crate::config_path());
    let mut directories = vec![settings.get("left_path"), settings.get("right_path")];
    if let Some(recents) = settings.get("recents").and_then(|value| value.as_array()) {
        directories.extend(recents.iter().map(Some));
    }
    let directories = directories
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(PathBuf::from)
        .collect::<std::collections::HashSet<_>>();
    let (tx, rx) = std::sync::mpsc::sync_channel(32);
    if LEGACY_MIGRATION_TX.set(tx).is_err() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("fileflow-backup-migration".into())
        .spawn(move || {
            if let Ok(sessions) = fs::read_dir(recovery_root()) {
                for session in sessions.flatten() {
                    let name = session.file_name().to_string_lossy().into_owned();
                    if legacy_backup_name(std::ffi::OsStr::new(&format!(".FileFlow-undo-{name}")))
                        && !name.starts_with(&format!("{}-", std::process::id()))
                        && session.file_type().is_ok_and(|kind| kind.is_dir())
                    {
                        cleanup_empty_backup(&session.path().join("files"));
                        let _ = fs::remove_dir(session.path()); // Empty only.
                    }
                }
            }
            let mut checked = std::collections::HashSet::new();
            for directory in directories.into_iter().chain(rx) {
                if !checked.insert(directory.clone()) {
                    continue;
                }
                match migrate_legacy_backups(&directory) {
                    Ok(count) if count > 0 => crate::integration_log(&format!(
                        "archived {count} legacy overwrite backups from {}",
                        directory.display()
                    )),
                    Err(error) => crate::integration_log(&format!(
                        "backup migration from {}: {error}",
                        directory.display()
                    )),
                    _ => {}
                }
            }
        });
}

static LEGACY_MIGRATION_TX: std::sync::OnceLock<std::sync::mpsc::SyncSender<PathBuf>> =
    std::sync::OnceLock::new();

pub fn request_legacy_backup_migration(directory: &Path) {
    if let Some(tx) = LEGACY_MIGRATION_TX.get() {
        let _ = tx.try_send(directory.to_path_buf());
    }
}

fn transfer_with_choice(
    kind: ShellOperationKind,
    sources: &[PathBuf],
    target: &Path,
    choose: impl FnOnce(&[PathBuf]) -> Result<Choice, String>,
) -> ShellOperationResult {
    assert!(matches!(
        kind,
        ShellOperationKind::Copy | ShellOperationKind::Move
    ));
    // Moving into the current directory is a no-op, including aliased paths.
    let sources = sources
        .iter()
        .filter(|source| {
            !matches!(kind, ShellOperationKind::Move)
                || !source
                    .file_name()
                    .is_some_and(|name| same_path(source, &target.join(name)))
        })
        .cloned()
        .collect::<Vec<_>>();
    let sources = sources.as_slice();
    if sources.is_empty() {
        return ShellOperationResult {
            result: Ok(()),
            moved: Vec::new(),
            copied: Vec::new(),
            replaced: Vec::new(),
        };
    }
    let conflicts = match collisions(sources, target) {
        Ok(found) => found,
        Err(e) => return empty_result(format!("无法检查目标同名项目：{e}")),
    };
    if conflicts.is_empty() {
        return perform_shell_file_operation(kind, sources, Some(target), None);
    }
    match choose(&conflicts) {
        Ok(Choice::Rename) => {
            return perform_shell_file_operation(kind, sources, Some(target), None);
        }
        Ok(Choice::Cancel) => return empty_result(format!("已取消{}", kind.progress_label())),
        Err(e) => return empty_result(e),
        Ok(Choice::Replace) => {}
    }
    // Do not move a destination ancestor that contains one of the sources.
    if conflicts.iter().any(|dest| {
        sources.iter().any(|src| {
            fs::canonicalize(src)
                .ok()
                .zip(fs::canonicalize(dest).ok())
                .is_some_and(|(src, dest)| src.starts_with(dest))
        })
    }) {
        return empty_result("源项目位于要覆盖的目标中，不能覆盖".into());
    }
    let dir = match backup_dir() {
        Ok(dir) => dir,
        Err(e) => return empty_result(format!("无法保留原内容，未覆盖：{e}")),
    };
    // Persist provenance before moving old data, including crash/partial-failure
    // recovery. Refuse to archive a directory containing the archive itself.
    if conflicts.iter().any(|dest| {
        fs::canonicalize(dest)
            .ok()
            .zip(fs::canonicalize(&dir).ok())
            .is_some_and(|(dest, dir)| dir.starts_with(dest))
    }) {
        cleanup_empty_backup(&dir);
        return empty_result("目标项目包含恢复目录，不能覆盖".into());
    }
    if let Err(error) = record_backups(&dir, target, kind.progress_label(), &conflicts) {
        cleanup_empty_backup(&dir);
        return empty_result(format!("无法记录原内容位置，未覆盖：{error}"));
    }
    let mut backups = Vec::new();
    let mut preparation_error = None;
    // Same-volume archival is an atomic rename; cross-volume archival uses Shell.
    for dest in &conflicts {
        // Keep original names so recovery remains understandable after a crash
        // or restart has discarded the in-memory undo record.
        let backup = dir.join(dest.file_name().expect("collision has a filename"));
        match archive_item(dest, &backup) {
            Ok(()) => backups.push(MoveRecord {
                from: backup,
                to: dest.clone(),
            }),
            Err(_)
                if fs::symlink_metadata(dest)
                    .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                    && !backup.exists() => {}
            Err(e) => {
                // Cross-volume archival may already hold a complete or partial
                // copy. Keep it tracked if recycling failed; never discard data.
                if backup.exists() {
                    backups.push(MoveRecord {
                        from: backup,
                        to: dest.clone(),
                    });
                }
                preparation_error = Some(format!("无法保留 {}，未覆盖：{e}", dest.display()));
                break;
            }
        }
    }
    let mut outcome = match preparation_error {
        Some(e) => empty_result(e),
        None => perform_shell_file_operation(kind, sources, Some(target), None),
    };
    // Even after consent, preserve RENAMEONCOLLISION in the Shell operation:
    // a new destination arriving after our backup must not be silently replaced.
    let (keep, rollback): (Vec<_>, Vec<_>) = backups.into_iter().partition(|backup| {
        outcome
            .copied
            .iter()
            .any(|copy| same_path(copy, &backup.to))
            || outcome
                .moved
                .iter()
                .any(|item| same_path(&item.from, &backup.to))
    });
    let restored = undo_moved_items(rollback);
    let mut retained = keep;
    if let Some(UndoEntry::Move(remaining)) = restored.remaining {
        retained.extend(remaining);
    }
    if !restored.errors.is_empty() {
        let previous = outcome.result.err().unwrap_or_default();
        outcome.result = Err(format!(
            "{previous}；原内容恢复未完成：{}",
            restored.errors.join("；")
        ));
    }
    outcome.replaced = retained;
    cleanup_empty_backup(&dir);
    outcome
}

pub fn undo_replaced_copy(copied: Vec<PathBuf>, backups: Vec<MoveRecord>) -> UndoResult {
    let mut copied_outcome = undo_copied_items(copied);
    let pending_copies = match copied_outcome.remaining.take() {
        Some(UndoEntry::Copy(paths)) => paths,
        _ => Vec::new(),
    };
    let parents = backups
        .iter()
        .filter_map(|b| b.from.parent().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    // Original paths still occupied (including failed removal) are not overwritten.
    let mut restore = undo_moved_items(backups);
    let pending_backups = match restore.remaining.take() {
        Some(UndoEntry::Move(items)) => items,
        _ => Vec::new(),
    };
    for dir in parents {
        cleanup_empty_backup(&dir);
    } // Empty generated dirs only.
    copied_outcome.errors.append(&mut restore.errors);
    UndoResult {
        restored: copied_outcome.restored + restore.restored,
        remaining: (!pending_copies.is_empty() || !pending_backups.is_empty()).then_some(
            UndoEntry::ReplaceCopy {
                copied: pending_copies,
                backups: pending_backups,
            },
        ),
        errors: copied_outcome.errors,
    }
}

pub fn undo_replaced_move(moved: Vec<MoveRecord>, backups: Vec<MoveRecord>) -> UndoResult {
    // Return incoming items to their source first. If that fails, their occupied
    // destination blocks backup restoration; both records remain retryable.
    let mut returned = undo_moved_items(moved);
    let pending_moves = match returned.remaining.take() {
        Some(UndoEntry::Move(items)) => items,
        _ => Vec::new(),
    };
    let parents = backups
        .iter()
        .filter_map(|b| b.from.parent().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    let mut restore = undo_moved_items(backups);
    let pending_backups = match restore.remaining.take() {
        Some(UndoEntry::Move(items)) => items,
        _ => Vec::new(),
    };
    for dir in parents {
        cleanup_empty_backup(&dir);
    }
    returned.errors.append(&mut restore.errors);
    UndoResult {
        restored: returned.restored + restore.restored,
        remaining: (!pending_moves.is_empty() || !pending_backups.is_empty()).then_some(
            UndoEntry::ReplaceMove {
                moved: pending_moves,
                backups: pending_backups,
            },
        ),
        errors: returned.errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_backups_migrate_without_losing_contents_or_original_paths() {
        let root = std::env::temp_dir().join(format!(
            "fileflow-legacy-backups-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let legacy = root.join(".FileFlow-undo-1234-1791665321241423700");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("旧文件.txt"), b"old-data").unwrap();
        fs::write(root.join("旧文件.txt"), b"new-data").unwrap();
        // A similarly named user folder must not be treated as a generated archive.
        let user_folder = root.join(".FileFlow-undo-not-generated");
        fs::create_dir(&user_folder).unwrap();
        fs::write(user_folder.join("keep.txt"), b"user-data").unwrap();
        let before = fs::read_dir(recovery_root())
            .unwrap_or_else(|_| {
                fs::create_dir_all(recovery_root()).unwrap();
                fs::read_dir(recovery_root()).unwrap()
            })
            .flatten()
            .map(|entry| entry.path())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(migrate_legacy_backups(&root).unwrap(), 1);
        assert!(!legacy.exists());
        assert_eq!(fs::read(root.join("旧文件.txt")).unwrap(), b"new-data");
        assert_eq!(
            fs::read(user_folder.join("keep.txt")).unwrap(),
            b"user-data"
        );
        let session = fs::read_dir(recovery_root())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| !before.contains(path))
            .find(|path| {
                fs::read(path.join("files").join("旧文件.txt"))
                    .is_ok_and(|bytes| bytes == b"old-data")
            })
            .expect("migrated data must remain archived");
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(session.join("record.json")).unwrap()).unwrap();
        assert_eq!(record["operation"], "legacy");
        assert_eq!(
            record["items"][0]["original_path"],
            root.join("旧文件.txt").to_string_lossy().as_ref()
        );
        let files = session.join("files");
        // Move into this fixture only so production cleanup can be verified
        // without discarding archived old bytes or overwriting the new file.
        fs::rename(files.join("旧文件.txt"), root.join("recovered.txt")).unwrap();
        cleanup_empty_backup(&files);
        assert!(!session.exists());
        assert_eq!(fs::read(root.join("recovered.txt")).unwrap(), b"old-data");
        assert_eq!(migrate_legacy_backups(&root).unwrap(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires FILEFLOW_RECOVERY_TEST_VOLUME_ROOT on a different volume than AppData"]
    fn cross_volume_backup_and_undo_preserve_whole_folders() {
        let volume = PathBuf::from(
            std::env::var_os("FILEFLOW_RECOVERY_TEST_VOLUME_ROOT").expect("explicit test volume"),
        );
        assert!(
            volume.is_absolute() && volume.parent().is_none(),
            "test volume must be a drive root"
        );
        let root = volume.join(format!(
            "FileFlow-backup-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&dest).unwrap();
        let incoming = source.join("folder");
        let existing = dest.join("folder");
        fs::create_dir(&incoming).unwrap();
        fs::create_dir(&existing).unwrap();
        fs::write(incoming.join("新图片.txt"), b"incoming").unwrap();
        fs::write(existing.join("旧图片.txt"), b"old").unwrap();
        fs::create_dir(existing.join("nested")).unwrap();
        fs::write(existing.join("nested").join("old.txt"), b"nested-old").unwrap();
        let replaced = copy_with_choice(std::slice::from_ref(&incoming), &dest, |_| {
            Ok(Choice::Replace)
        });
        assert!(replaced.result.is_ok(), "{:?}", replaced.result);
        assert_eq!(replaced.replaced.len(), 1);
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
        let backup = &replaced.replaced[0].from;
        assert!(backup.starts_with(recovery_root()) && !backup.starts_with(&root));
        assert_eq!(fs::read(backup.join("旧图片.txt")).unwrap(), b"old");
        assert_eq!(
            fs::read(backup.join("nested").join("old.txt")).unwrap(),
            b"nested-old"
        );
        let session = backup.parent().unwrap().parent().unwrap().to_path_buf();
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(session.join("record.json")).unwrap()).unwrap();
        assert_eq!(
            record["items"][0]["original_path"],
            existing.to_string_lossy().as_ref()
        );
        let undone = undo_replaced_copy(replaced.copied, replaced.replaced);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(fs::read(existing.join("旧图片.txt")).unwrap(), b"old");
        assert_eq!(
            fs::read(existing.join("nested").join("old.txt")).unwrap(),
            b"nested-old"
        );
        assert_eq!(fs::read(incoming.join("新图片.txt")).unwrap(), b"incoming");
        assert!(!existing.join("新图片.txt").exists() && !session.exists());
        fs::remove_dir_all(root).unwrap();
    }
    fn copy_with_choice(
        sources: &[PathBuf],
        target: &Path,
        choose: impl FnOnce(&[PathBuf]) -> Result<Choice, String>,
    ) -> ShellOperationResult {
        transfer_with_choice(ShellOperationKind::Copy, sources, target, choose)
    }
    #[test]
    fn move_collision_choices_and_retryable_undo_preserve_both_files() {
        let root = std::env::temp_dir().join(format!(
            "fileflow-move-conflicts-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&dest).unwrap();
        let file = source.join("中文.txt");
        let existing = dest.join("中文.txt");
        fs::write(&file, b"incoming").unwrap();
        fs::write(&existing, b"original").unwrap();
        let move_with = |choice| {
            transfer_with_choice(
                ShellOperationKind::Move,
                std::slice::from_ref(&file),
                &dest,
                |_| Ok(choice),
            )
        };
        let unchanged = transfer_with_choice(
            ShellOperationKind::Move,
            std::slice::from_ref(&file),
            &source,
            |_| panic!("same-directory move must not prompt"),
        );
        assert!(unchanged.result.is_ok() && unchanged.moved.is_empty());
        assert_eq!(fs::read_dir(&source).unwrap().count(), 1);
        let canceled = move_with(Choice::Cancel);
        assert_eq!(canceled.result.unwrap_err(), "已取消移动");
        assert!(canceled.moved.is_empty() && canceled.replaced.is_empty());
        assert_eq!(fs::read(&file).unwrap(), b"incoming");
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        let renamed = move_with(Choice::Rename);
        assert!(renamed.result.is_ok(), "{:?}", renamed.result);
        assert!(!file.exists());
        assert_eq!(renamed.moved.len(), 1);
        assert_ne!(renamed.moved[0].from, existing);
        assert_eq!(fs::read(&renamed.moved[0].from).unwrap(), b"incoming");
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        let undone = undo_moved_items(renamed.moved);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(fs::read(&file).unwrap(), b"incoming");
        let replaced = move_with(Choice::Replace);
        assert!(replaced.result.is_ok(), "{:?}", replaced.result);
        assert_eq!(replaced.moved.len(), 1);
        assert_eq!(replaced.replaced.len(), 1);
        assert!(replaced.replaced[0].from.starts_with(recovery_root()));
        assert_eq!(
            fs::read_dir(&dest).unwrap().count(),
            1,
            "archives must stay out of the working folder"
        );
        assert!(!file.exists());
        assert_eq!(fs::read(&existing).unwrap(), b"incoming");
        assert_eq!(fs::read(&replaced.replaced[0].from).unwrap(), b"original");
        // A new file at the source must not be overwritten by undo. Retain both
        // the incoming file and original destination until the source is free.
        fs::write(&file, b"new-unrelated-source").unwrap();
        let blocked = undo_replaced_move(replaced.moved, replaced.replaced);
        assert!(!blocked.errors.is_empty());
        let Some(UndoEntry::ReplaceMove { moved, backups }) = blocked.remaining else {
            panic!("replacement undo must remain a single retryable record");
        };
        assert_eq!(moved.len(), 1);
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&file).unwrap(), b"new-unrelated-source");
        assert_eq!(fs::read(&existing).unwrap(), b"incoming");
        assert_eq!(fs::read(&backups[0].from).unwrap(), b"original");
        fs::rename(&file, source.join("unrelated-kept.txt")).unwrap();
        let undone = undo_replaced_move(moved, backups);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(fs::read(&file).unwrap(), b"incoming");
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
        let failed = transfer_with_choice(
            ShellOperationKind::Move,
            &[file.clone(), source.join("missing.txt")],
            &dest,
            |_| Ok(Choice::Replace),
        );
        assert!(failed.result.is_err());
        assert!(failed.moved.is_empty() && failed.replaced.is_empty());
        assert_eq!(fs::read(&file).unwrap(), b"incoming");
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
        // Folder replacement returns the incoming folder and restores all old
        // target contents, including files absent from the incoming folder.
        let folder = source.join("folder");
        let target_folder = dest.join("folder");
        fs::create_dir(&folder).unwrap();
        fs::create_dir(&target_folder).unwrap();
        fs::write(folder.join("new.txt"), b"new").unwrap();
        fs::write(target_folder.join("old.txt"), b"old").unwrap();
        let replaced = transfer_with_choice(
            ShellOperationKind::Move,
            std::slice::from_ref(&folder),
            &dest,
            |_| Ok(Choice::Replace),
        );
        assert!(replaced.result.is_ok(), "{:?}", replaced.result);
        assert!(!folder.exists() && !target_folder.join("old.txt").exists());
        let undone = undo_replaced_move(replaced.moved, replaced.replaced);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(fs::read(folder.join("new.txt")).unwrap(), b"new");
        assert_eq!(fs::read(target_folder.join("old.txt")).unwrap(), b"old");
        assert!(!target_folder.join("new.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancel_rename_replace_and_failed_copy_preserve_original_contents() {
        let root = std::env::temp_dir().join(format!(
            "fileflow-copy-conflicts-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&dest).unwrap();
        let file = source.join("中文.txt");
        let existing = dest.join("中文.txt");
        fs::write(&file, b"new-source").unwrap();
        fs::write(&existing, b"old-target").unwrap();
        assert_eq!(
            collisions(std::slice::from_ref(&file), &source).unwrap(),
            Vec::<PathBuf>::new()
        );
        let canceled = copy_with_choice(std::slice::from_ref(&file), &dest, |_| Ok(Choice::Cancel));
        assert_eq!(canceled.result.unwrap_err(), "已取消复制");
        assert!(canceled.copied.is_empty() && canceled.replaced.is_empty());
        assert_eq!(fs::read(&existing).unwrap(), b"old-target");
        let renamed = copy_with_choice(std::slice::from_ref(&file), &dest, |_| Ok(Choice::Rename));
        assert!(renamed.result.is_ok(), "{:?}", renamed.result);
        assert_eq!(renamed.copied.len(), 1);
        assert_ne!(renamed.copied[0], existing);
        assert_eq!(fs::read(&renamed.copied[0]).unwrap(), b"new-source");
        assert_eq!(fs::read(&existing).unwrap(), b"old-target");
        assert!(undo_copied_items(renamed.copied).errors.is_empty());
        let replaced =
            copy_with_choice(std::slice::from_ref(&file), &dest, |_| Ok(Choice::Replace));
        assert!(replaced.result.is_ok(), "{:?}", replaced.result);
        assert_eq!(replaced.copied.len(), 1);
        assert_eq!(replaced.replaced.len(), 1);
        assert!(replaced.replaced[0].from.starts_with(recovery_root()));
        assert_eq!(
            fs::read_dir(&dest).unwrap().count(),
            1,
            "archives must stay out of the working folder"
        );
        assert_eq!(fs::read(&existing).unwrap(), b"new-source");
        assert_eq!(fs::read(&file).unwrap(), b"new-source");
        assert_eq!(fs::read(&replaced.replaced[0].from).unwrap(), b"old-target");
        let undone = undo_replaced_copy(replaced.copied, replaced.replaced);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(fs::read(&existing).unwrap(), b"old-target");
        let failed = copy_with_choice(&[file.clone(), source.join("missing.txt")], &dest, |_| {
            Ok(Choice::Replace)
        });
        assert!(failed.result.is_err());
        assert!(failed.copied.is_empty() && failed.replaced.is_empty());
        assert_eq!(fs::read(&existing).unwrap(), b"old-target");
        assert_eq!(
            fs::read_dir(&dest).unwrap().count(),
            1,
            "empty recovery directories must be removed"
        );
        // Locked targets must survive without a copy or orphan backup.
        use std::os::windows::fs::OpenOptionsExt;
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&existing)
            .unwrap();
        let blocked = copy_with_choice(std::slice::from_ref(&file), &dest, |_| Ok(Choice::Replace));
        assert!(blocked.result.is_err());
        assert!(blocked.copied.is_empty() && blocked.replaced.is_empty());
        drop(locked);
        assert_eq!(fs::read(&existing).unwrap(), b"old-target");
        // Whole-folder replacement is explicit in the dialog; undo restores the
        // original directory, including files absent from the source folder.
        let folder = source.join("folder");
        let old_folder = dest.join("folder");
        fs::create_dir(&folder).unwrap();
        fs::create_dir(&old_folder).unwrap();
        fs::write(folder.join("new.txt"), b"new-folder-content").unwrap();
        fs::write(old_folder.join("old.txt"), b"old-folder-content").unwrap();
        let replaced = copy_with_choice(std::slice::from_ref(&folder), &dest, |_| {
            Ok(Choice::Replace)
        });
        assert!(replaced.result.is_ok(), "{:?}", replaced.result);
        assert!(!old_folder.join("old.txt").exists());
        assert_eq!(
            fs::read(old_folder.join("new.txt")).unwrap(),
            b"new-folder-content"
        );
        let undone = undo_replaced_copy(replaced.copied, replaced.replaced);
        assert!(
            undone.errors.is_empty() && undone.remaining.is_none(),
            "{:?}",
            undone.errors
        );
        assert_eq!(
            fs::read(old_folder.join("old.txt")).unwrap(),
            b"old-folder-content"
        );
        assert!(!old_folder.join("new.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
