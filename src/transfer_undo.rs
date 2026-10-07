//! FileFlow 的传输撤销记录来自 Shell 完成回调，不能用“目标目录 + 原名”猜测。
use super::{ShellOperationKind, UndoEntry, perform_shell_file_operation};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem, SICHINT_CANONICAL,
    SIGDN_FILESYSPATH,
};
use windows::core::{Ref, Result as WResult, implement};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MoveRecord {
    pub from: PathBuf,
    pub to: PathBuf,
}

pub(super) struct ShellOperationResult {
    pub result: Result<(), String>,
    pub moved: Vec<MoveRecord>,
    pub copied: Vec<PathBuf>,
}

pub(super) struct UndoResult {
    pub restored: usize,
    pub remaining: Option<UndoEntry>,
    pub errors: Vec<String>,
}

#[implement(IFileOperationProgressSink)]
pub(super) struct TransferProgress {
    original: PathBuf,
    original_item: IShellItem,
    moved: Arc<Mutex<Vec<MoveRecord>>>,
    copied: Arc<Mutex<Vec<PathBuf>>>,
}

impl TransferProgress {
    pub fn new(
        original: PathBuf,
        original_item: IShellItem,
        moved: Arc<Mutex<Vec<MoveRecord>>>,
        copied: Arc<Mutex<Vec<PathBuf>>>,
    ) -> Self {
        Self {
            original,
            original_item,
            moved,
            copied,
        }
    }
}

fn completed_path(item: Ref<'_, IShellItem>) -> WResult<Option<PathBuf>> {
    let Some(item) = item.as_ref() else {
        return Ok(None);
    };
    // GetDisplayName 返回 COM 分配的 UTF-16 字符串，转换后必须释放。
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(std::ffi::OsString::from_wide(name.as_wide()));
        CoTaskMemFree(Some(name.0.cast()));
        Ok(Some(path))
    }
}

pub(super) fn undo_moved_items(items: Vec<MoveRecord>) -> UndoResult {
    let mut restored = 0;
    let mut remaining = Vec::new();
    let mut errors = Vec::new();
    for mut item in items {
        let problem = if !item.from.exists() {
            Some(format!("待恢复文件不存在：{}", item.from.display()))
        } else if item.to.exists() {
            Some(format!("原位置已有同名项目，未覆盖：{}", item.to.display()))
        } else if !item.to.parent().is_some_and(|parent| parent.is_dir()) {
            Some(format!("原目录不存在：{}", item.to.display()))
        } else {
            None
        };
        if let Some(problem) = problem {
            errors.push(problem);
            remaining.push(item);
            continue;
        }
        let outcome = perform_shell_file_operation(
            ShellOperationKind::Move,
            std::slice::from_ref(&item.from),
            item.to.parent(),
            item.to.file_name(),
        );
        if let Some(done) = outcome.moved.first() {
            // Shell 回调返回长路径；原路径可能含 ADMINI~1 等 8.3 短名。
            let restored_to_original = done.from == item.to
                || std::fs::canonicalize(&done.from)
                    .ok()
                    .zip(std::fs::canonicalize(&item.to).ok())
                    .is_some_and(|(actual, original)| actual == original);
            if restored_to_original {
                restored += 1;
                continue;
            }
            // 检查后又发生重名时 Shell 会自动改名；保留实际路径供再次撤销。
            item.from = done.from.clone();
        }
        errors.push(
            outcome
                .result
                .err()
                .unwrap_or_else(|| format!("未恢复到原位置：{}", item.to.display())),
        );
        remaining.push(item);
    }
    UndoResult {
        restored,
        remaining: (!remaining.is_empty()).then_some(UndoEntry::Move(remaining)),
        errors,
    }
}

pub(super) fn undo_copied_items(paths: Vec<PathBuf>) -> UndoResult {
    let mut restored = 0;
    let mut remaining = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        if !path.exists() {
            // 副本已被移走或删除，不能猜测其它位置，更不能操作原文件。
            restored += 1;
            continue;
        }
        let outcome = perform_shell_file_operation(
            ShellOperationKind::Delete,
            std::slice::from_ref(&path),
            None,
            None,
        );
        if outcome.result.is_ok() && !path.exists() {
            restored += 1;
        } else {
            errors.push(
                outcome
                    .result
                    .err()
                    .unwrap_or_else(|| format!("副本未移入回收站：{}", path.display())),
            );
            remaining.push(path);
        }
    }
    UndoResult {
        restored,
        remaining: (!remaining.is_empty()).then_some(UndoEntry::Copy(remaining)),
        errors,
    }
}

#[allow(unused_variables)]
impl IFileOperationProgressSink_Impl for TransferProgress_Impl {
    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, hrresult: windows::core::HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
        hrrename: windows::core::HRESULT,
        psinewlycreated: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
        hrmove: windows::core::HRESULT,
        psinewlycreated: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        // Shell 也会回调目录内的子项。只记录调用 MoveItem/CopyItem 时的顶层项。
        if hrmove.is_ok()
            && psiitem.as_ref().is_some_and(|item| unsafe {
                self.original_item
                    .Compare(item, SICHINT_CANONICAL.0 as u32)
                    .is_ok_and(|order| order == 0)
            })
            && let Some(path) = completed_path(psinewlycreated)?
            && path != self.original
        {
            self.moved.lock().unwrap().push(MoveRecord {
                from: path,
                to: self.original.clone(),
            });
        }
        Ok(())
    }
    fn PreCopyItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
        hrcopy: windows::core::HRESULT,
        psinewlycreated: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        if hrcopy.is_ok()
            && psiitem.as_ref().is_some_and(|item| unsafe {
                self.original_item
                    .Compare(item, SICHINT_CANONICAL.0 as u32)
                    .is_ok_and(|order| order == 0)
            })
            && let Some(path) = completed_path(psinewlycreated)?
            && path != self.original
        {
            self.copied.lock().unwrap().push(path);
        }
        Ok(())
    }
    fn PreDeleteItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        dwflags: u32,
        psiitem: windows::core::Ref<'_, IShellItem>,
        hrdelete: windows::core::HRESULT,
        psinewlycreated: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(
        &self,
        dwflags: u32,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        dwflags: u32,
        psidestinationfolder: windows::core::Ref<'_, IShellItem>,
        psznewname: &windows::core::PCWSTR,
        psztemplatename: &windows::core::PCWSTR,
        dwfileattributes: u32,
        hrnew: windows::core::HRESULT,
        psinewitem: windows::core::Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, iworktotal: u32, iworksofar: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn shell_transfers_record_actual_paths_and_undo_without_overwriting() {
        let root = std::env::temp_dir().join(format!(
            "fileflow-transfer-undo-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&dest).unwrap();
        let a = source.join("中文.txt");
        let folder = source.join("文件夹");
        fs::write(&a, b"original-file").unwrap();
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("nested.txt"), b"nested-file").unwrap();
        let occupied = dest.join("中文.txt");
        fs::write(&occupied, b"keep-existing-destination").unwrap();

        // A single drag containing a file and a folder; destination collision changes the filename.
        let moved = perform_shell_file_operation(
            ShellOperationKind::Move,
            &[a.clone(), folder.clone()],
            Some(&dest),
            None,
        );
        assert!(moved.result.is_ok(), "{:?}", moved.result);
        assert_eq!(moved.moved.len(), 2);
        assert!(!a.exists() && !folder.exists());
        let actual_a = moved.moved.iter().find(|m| m.to == a).unwrap().from.clone();
        assert_ne!(actual_a, occupied);
        assert_eq!(fs::read(&actual_a).unwrap(), b"original-file");
        assert_eq!(fs::read(&occupied).unwrap(), b"keep-existing-destination");

        // A conflicting original must survive. The other item can be restored, with retry retained.
        fs::write(&a, b"keep-new-original").unwrap();
        let partial = undo_moved_items(moved.moved);
        assert_eq!(partial.restored, 1, "errors={:?}", partial.errors);
        assert!(!partial.errors.is_empty());
        assert_eq!(fs::read(&a).unwrap(), b"keep-new-original");
        assert_eq!(fs::read(folder.join("nested.txt")).unwrap(), b"nested-file");
        let Some(UndoEntry::Move(pending)) = partial.remaining else {
            panic!("failed item must remain undoable")
        };
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].from, actual_a);
        fs::remove_file(&a).unwrap();
        let retried = undo_moved_items(pending);
        assert_eq!(retried.restored, 1);
        assert!(retried.remaining.is_none() && retried.errors.is_empty());
        assert_eq!(fs::read(&a).unwrap(), b"original-file");
        assert!(!actual_a.exists());

        // Copy/paste into the same folder generates a new name. Undo only recycles that copy.
        let copied = perform_shell_file_operation(
            ShellOperationKind::Copy,
            &[a.clone(), folder.clone()],
            Some(&source),
            None,
        );
        assert!(copied.result.is_ok(), "{:?}", copied.result);
        assert_eq!(copied.copied.len(), 2);
        let duplicate = copied.copied.iter().find(|p| p.is_file()).unwrap().clone();
        let duplicate_folder = copied.copied.iter().find(|p| p.is_dir()).unwrap().clone();
        assert_eq!(
            fs::read(duplicate_folder.join("nested.txt")).unwrap(),
            b"nested-file"
        );
        assert_ne!(duplicate, a);
        assert_eq!(fs::read(&duplicate).unwrap(), b"original-file");
        let undone_copy = undo_copied_items(copied.copied);
        assert_eq!(undone_copy.restored, 2);
        assert!(
            undone_copy.remaining.is_none() && undone_copy.errors.is_empty(),
            "{:?}",
            undone_copy.errors
        );
        assert!(!duplicate.exists() && !duplicate_folder.exists());
        assert_eq!(fs::read(folder.join("nested.txt")).unwrap(), b"nested-file");
        assert_eq!(fs::read(&a).unwrap(), b"original-file");
        assert_eq!(fs::read(&occupied).unwrap(), b"keep-existing-destination");

        // A missing source fails before executing: it must not fabricate an undo record.
        let failed = perform_shell_file_operation(
            ShellOperationKind::Move,
            &[source.join("missing.txt")],
            Some(&dest),
            None,
        );
        assert!(failed.result.is_err());
        assert!(failed.moved.is_empty() && failed.copied.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
