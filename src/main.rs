#![windows_subsystem = "windows"]

mod text_input;

use chrono::{DateTime, Local};
use gpui::StatefulInteractiveElement;
use gpui::prelude::*;
use gpui::{AsyncApp,
    UniformListScrollHandle,
    Animation, AnimationExt, AnyElement, App, Application, Bounds, ClickEvent, ClipboardItem,
    Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding, Keystroke, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, NavigationDirection, ObjectFit, Pixels, Point,
    Render, ScrollDelta, ScrollStrategy, ScrollWheelEvent, TitlebarOptions,
    Window, WindowBounds, WindowControlArea, WindowOptions, actions, div, img, px, rgb, rgba,
    size, uniform_list,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Write};
use std::mem::ManuallyDrop;
use std::net::{TcpListener, TcpStream};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicIsize, AtomicU32, AtomicUsize, Ordering as AtomicOrdering,
};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use text_input::TextInput;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC, E_NOTIMPL,
    GlobalFree, HANDLE as WHANDLE, HWND as WHWND, POINT as WPOINT, RPC_E_CHANGED_MODE, S_OK,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IBindCtx,
    IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0,
    TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DoDragDrop,
    IDropSource, IDropSource_Impl, OleFlushClipboard, OleInitialize, OleSetClipboard,
    OleUninitialize,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    CMF_EXTENDEDVERBS, CMF_NORMAL, CMINVOKECOMMANDINFO, DROPFILES, DragQueryFileW,
    FILEOPERATION_FLAGS, FOF_ALLOWUNDO, FOF_NOCONFIRMMKDIR, FOF_RENAMEONCOLLISION,
    FOFX_ADDUNDORECORD,
    FOFX_RECYCLEONDELETE, FileOperation, HDROP, IContextMenu, IFileOperation,
    IFileOperationProgressSink, ILFindLastID, IShellFolder, IShellItem, SHBindToObject,
    SHBindToParent, SHCreateDataObject, SHCreateItemFromParsingName, SHCreateStdEnumFmtEtc,
    SHParseDisplayName, SHCONTF_FOLDERS, SHCONTF_NONFOLDERS, SHGDN_FORPARSING, StrRetToBufW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu as WCreatePopupMenu, DestroyMenu as WDestroyMenu,
    GetCursorPos as WGetCursorPos, SW_SHOWNORMAL as WSW_SHOWNORMAL,
    TPM_LEFTALIGN as WTPM_LEFTALIGN, TPM_RETURNCMD as WTPM_RETURNCMD,
    TPM_RIGHTBUTTON as WTPM_RIGHTBUTTON, TPM_TOPALIGN as WTPM_TOPALIGN,
    TrackPopupMenu as WTrackPopupMenu,
};
use windows::core::{
    Error as WError, PCSTR as WPCSTR, PCWSTR as WPCWSTR, Result as WResult, implement,
};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, HWND, LPARAM, LRESULT,
    POINT, WPARAM,
};
use windows_sys::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
use windows_sys::Win32::Storage::FileSystem::{
    GetLogicalDrives, NetShareEnum, SHARE_INFO_1, STYPE_DISKTREE, STYPE_MASK,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, CreateMutexW, GetCurrentProcessId, GetCurrentThreadId,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetDoubleClickTime, ReleaseCapture, SetCapture, VK_E, VK_LBUTTON, VK_LWIN,
    VK_RWIN,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICON_VERSION_4,
    NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, AppendMenuW, BringWindowToTop, CallNextHookEx, CreatePopupMenu,
    CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, EnumThreadWindows,
    EnumWindows, FindWindowW, GetCursorPos, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowTextW, GetWindowThreadProcessId, HC_ACTION, IDI_APPLICATION, IsWindowVisible, IsZoomed,
    KBDLLHOOKSTRUCT, LoadIconW, MF_STRING, MSG, PM_REMOVE, PeekMessageW, PostMessageW, PostQuitMessage,
    CallWindowProcW, GWLP_WNDPROC, HTCAPTION, WM_CANCELMODE, WM_CAPTURECHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN,
    WM_NCLBUTTONUP, WM_NCMOUSEMOVE, WM_NCHITTEST, WNDPROC, SetWindowLongPtrW,
    RegisterClassW, SW_HIDE, SW_MAXIMIZE, SW_RESTORE, SW_SHOWNORMAL, SM_CXDOUBLECLK,
    SM_CYDOUBLECLK, SetForegroundWindow,
    SetWindowsHookExW, ShowWindow, ShowWindowAsync, SwitchToThisWindow, IsIconic, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_CONTEXTMENU, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_QUIT, WM_RBUTTONUP,
    WM_SYSKEYDOWN, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const BLUE: u32 = 0x00ADEE;
const HOVER_BLUE: u32 = 0xABE3F7;
const TEXT: u32 = 0x2F3439;
const MUTED: u32 = 0x8A9299;
const THIS_PC_PATH: &str = "This PC";
const NETWORK_PATH: &str = "Network";
const RECYCLE_BIN_PATH: &str = "Recycle Bin";
static WIN_E_ENABLED: AtomicBool = AtomicBool::new(false);
/// DoDragDrop 原生模态循环进行中：gpui 后台任务必须暂缓 view.update，
/// 否则对已借用的 App 再 borrow_mut 会 panic（async_context.rs RefCell）
static OLE_MODAL_ACTIVE: AtomicBool = AtomicBool::new(false);
/// 标题带阈值拖动状态（drag_zone_wndproc 内使用）：
/// NC 按下先吞掉缓存，移动超阈值才启动系统移动循环；否则松手时把
/// down+up 合成普通客户区点击转发给 gpui（标签/按钮点击不受影响）。
static CAPTION_PENDING: AtomicBool = AtomicBool::new(false);
static CAPTION_DRAGGING: AtomicBool = AtomicBool::new(false);
static CAPTION_DOWN_X: AtomicI32 = AtomicI32::new(0);
static CAPTION_DOWN_Y: AtomicI32 = AtomicI32::new(0);
/// 双击标题带已处理（最大化/还原）：丢弃随后的抬起，避免孤立 MouseUp 进 gpui
static CAPTION_SKIP_UP: AtomicBool = AtomicBool::new(false);
/// 上一次标题带按下 tick（GetTickCount）。双击判定兜底：系统未把第二次按下
/// 合成 WM_NCLBUTTONDBLCLK 时，在按下阶段按双击时间+距离手动切换最大化/还原。
/// 位置复用 CAPTION_DOWN_X/Y（按下时先比对旧值再覆盖）。
static CAPTION_LAST_DOWN_TICK: AtomicU32 = AtomicU32::new(0);
/// 托盘右键菜单进行中：v4 格式下 WM_RBUTTONUP 与 WM_CONTEXTMENU 可能连发，防弹两次
static TRAY_MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// 标题栏拖动区命中带：subclass 窗口过程对 WM_NCHITTEST 返回 HTCAPTION。
/// gpui 的 WindowControlArea::Drag 在外部环境不可靠，改为固定几何：
/// 标签行（0~42 逻辑 px，顶部 8px 让给缩放边），x 排除 ☰ 与窗口控制按钮。
/// 窗口 DPI 缩放（f32 以 bits 存入 AtomicU32），install 时填。
static DRAG_SCALE: AtomicU32 = AtomicU32::new(1_004_000); // 1.0 的 f32 bits 兜底
static WIN_E_LISTENER_STARTED: AtomicBool = AtomicBool::new(false);
static TRAY_LISTENER_STARTED: AtomicBool = AtomicBool::new(false);
static FILEFLOW_HWND: AtomicIsize = AtomicIsize::new(0);
static THUMBNAIL_REQUEST_TX: OnceLock<async_channel::Sender<PathBuf>> = OnceLock::new();
/// 文件夹递归大小缓存（后台算完写入，渲染零 IO）
static FOLDER_SIZE_CACHE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
/// 文件夹大小计算请求通道（去重由 worker 侧 pending 集合保证）
static FOLDER_SIZE_TX: OnceLock<async_channel::Sender<PathBuf>> = OnceLock::new();
/// 文件夹大小计算完成通知（触发 UI 刷新）
static FOLDER_SIZE_DONE_TX: OnceLock<async_channel::Sender<PathBuf>> = OnceLock::new();
static THUMBNAIL_PENDING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
static THUMBNAIL_FAILED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
/// 后台任务→整窗重绘的节流通道（缩略图完成、路径状态验证完成共用）
static UI_REFRESH_TX: OnceLock<async_channel::Sender<PathBuf>> = OnceLock::new();
/// 渲染线程零 IO 的路径状态缓存：1=目录 2=非目录/不存在（后台 stat 填充）
static PATH_STATUS: OnceLock<Mutex<HashMap<PathBuf, u8>>> = OnceLock::new();
static PATH_STATUS_PENDING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
/// 文件类型图标的会话级记忆：(扩展名, 尺寸桶) → PNG 路径（None=提取失败，不重试）
type FiletypeIconMemo = Mutex<HashMap<(String, u32), Option<PathBuf>>>;
static FILETYPE_ICON_MEMO: OnceLock<FiletypeIconMemo> = OnceLock::new();
/// 缩略图缓存盘上是否已存在的会话级记忆（渲染帧不反复 stat）；
/// 后台生成成功后由 worker 置 true
static THUMBNAIL_CACHE_MEMO: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
const WM_FILEFLOW_TRAY: u32 = WM_USER + 0x51;
const TRAY_ICON_ID: u32 = 1;
const TRAY_OPEN_ID: usize = 1001;
const TRAY_EXIT_ID: usize = 1002;
const FILEFLOW_COMMAND_ADDR: &str = "127.0.0.1:47719";

actions!(
    fileflow,
    [
        Refresh,
        GoUp,
        GoBack,
        GoForward,
        ToggleSplit,
        ToggleSidebar,
        NextView,
        PreviousView,
        NewTab,
        CloseTab,
        NewFolder,
        NewTextFile,
        ToggleHidden,
        CycleFolders,
        CopyToOther,
        MoveToOther,
        CopyPaths,
        FocusFilter,
        FocusAddress,
        ClearTransient,
        SubmitAddress,
        OpenSettings,
        OpenShortcuts,
        TogglePreview,
        ToggleFavorite,
        SelectAllEntries,
        OpenProperties,
        CopySelected,
        CutSelected,
        PasteFiles,
        RenameSelected,
        DeleteSelected,
        /// Shift+Del：直接永久删除（不进回收站、不可撤销）
        PermanentDeleteSelected,
        Undo,
        NavigateUp,
        NavigateDown,
        NavigateLeft,
        NavigateRight,
        ViewDetails,
        ViewList,
        ViewColumns,
        ViewMIcons,
        ViewLIcons,
        ViewXLIcons
    ]
);

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: String,
    /// 加载时的修改时间毫秒数，用于缩略图缓存 key（渲染时零磁盘 IO）
    modified_millis: u64,
}

enum WatchCommand {
    SetPaths(Vec<PathBuf>),
}

#[derive(Clone, Copy)]
enum ShellOperationKind {
    Copy,
    Move,
    /// Delete：进回收站（可 Ctrl+Z 撤销）
    Delete,
    /// PermanentDelete：Shift+Del 直接抹除，不进回收站、不可撤销
    PermanentDelete,
}

/// Ctrl+Z 撤销记录：把 from 路径恢复为 to 路径（新建 = to 为 None）
#[derive(Clone)]
enum UndoEntry {
    /// 删除（回收站）：恢复这些路径
    Restore(Vec<PathBuf>),
    /// 重命名：改回旧名
    Rename { from: PathBuf, to: PathBuf },
    /// 批量重命名：from/to 一一对应改回
    RenameBatch { from: Vec<PathBuf>, to: Vec<PathBuf> },
    /// 新建文件夹/文件：删除恢复
    RemoveNew(Vec<PathBuf>),
}

impl ShellOperationKind {
    fn progress_label(self) -> &'static str {
        match self {
            Self::Copy => "复制",
            Self::Move => "移动",
            Self::Delete => "移入回收站",
            Self::PermanentDelete => "永久删除",
        }
    }
}

struct CrumbTooltip {
    text: String,
}

impl Render for CrumbTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_lg()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xcfd6db))
            .shadow_lg()
            .text_color(rgb(TEXT))
            .child(self.text.clone())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Details,
    List,
    Columns,
    MIcons,
    LIcons,
    XLIcons,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderOrder {
    FoldersOnly,
    Mixed,
    FoldersFirst,
    FilesFirst,
}

impl FolderOrder {
    fn label(self) -> &'static str {
        match self {
            Self::FoldersOnly => "仅文件夹",
            Self::Mixed => "文件和文件夹混合",
            Self::FoldersFirst => "文件夹优先",
            Self::FilesFirst => "文件优先",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::FoldersOnly => Self::Mixed,
            Self::Mixed => Self::FoldersFirst,
            Self::FoldersFirst => Self::FilesFirst,
            Self::FilesFirst => Self::FoldersOnly,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Name,
    Type,
    Modified,
    Size,
}

#[derive(Clone)]
struct DragSelectState {
    side: &'static str,
    start: Point<Pixels>,
    current: Point<Pixels>,
    visible_paths: Vec<PathBuf>,
    modifiers: gpui::Modifiers,
    active: bool,
}

#[derive(Clone)]
struct FileDragState {
    start: Point<Pixels>,
    current: Point<Pixels>,
    sources: Vec<PathBuf>,
    modifiers: gpui::Modifiers,
    active: bool,
    external_started: bool,
}

#[derive(Clone)]
struct FileDragPayload {
    sources: Vec<PathBuf>,
}

struct FileDragPreview {
    label: String,
    position: Point<Pixels>,
}

impl Render for FileDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .pl(self.position.x - px(20.))
            .pt(self.position.y - px(18.))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .shadow_lg()
                    .bg(rgba(0x00ADEECC))
                    .text_color(rgb(0xffffff))
                    .child(self.label.clone()),
            )
    }
}

impl ViewMode {
    fn label(self) -> &'static str {
        match self {
            Self::Details => "Details",
            Self::List => "List",
            Self::Columns => "Columns",
            Self::MIcons => "M Icons",
            Self::LIcons => "L Icons",
            Self::XLIcons => "XL Icons",
        }
    }
}

struct FileFlowGpui {
    tabs: Vec<PathBuf>,
    active_tab: usize,
    left_back: Vec<PathBuf>,
    left_forward: Vec<PathBuf>,
    right_back: Vec<PathBuf>,
    right_forward: Vec<PathBuf>,
    active_side: &'static str,
    split: bool,
    right_path: PathBuf,
    right_tabs: Vec<PathBuf>,
    right_active_tab: usize,
    left_entries: Vec<Entry>,
    /// 渲染视图数据缓存：arranged_entries（过滤+排序）后的结果，
    /// uniform_list 行构建闭包异步读取，保证与当前帧数据一致
    left_display_entries: Vec<Entry>,
    right_entries: Vec<Entry>,
    /// 渲染视图数据缓存（右面板，同 left_display_entries）
    right_display_entries: Vec<Entry>,
    left_view_mode: ViewMode,
    right_view_mode: ViewMode,
    status: String,
    folder_order: FolderOrder,
    show_hidden: bool,
    show_sidebar: bool,
    focus_handle: FocusHandle,
    left_scroll_handle: UniformListScrollHandle,
    right_scroll_handle: UniformListScrollHandle,
    left_selected: HashSet<PathBuf>,
    right_selected: HashSet<PathBuf>,
    left_anchor: Option<PathBuf>,
    right_anchor: Option<PathBuf>,
    left_focused: Option<PathBuf>,
    right_focused: Option<PathBuf>,
    drag_select: Option<DragSelectState>,
    file_drag: Option<FileDragState>,
    suppress_blank_click: bool,
    view_epoch: u64,
    filter_input: Entity<TextInput>,
    right_filter_input: Entity<TextInput>,
    address_input: Entity<TextInput>,
    address_suggestions: Vec<PathBuf>,
    address_suggestion_generation: u64,
    /// 点击地址栏进入编辑时置位：跳过 set_value 触发的那轮建议探测
    address_suggestions_suppress: bool,
    address_editing: bool,
    address_side: &'static str,
    ui_font_size: f32,
    file_font_size: f32,
    row_spacing: f32,
    load_thumbnails: bool,
    win_e_enabled: bool,
    /// 替换资源管理器：文件夹/驱动器默认打开方式改为 FileFlow
    explorer_replacement: bool,
    /// 本次布局的窗口逻辑宽度（render 写入，列宽自适应读取）。
    ///
    /// 列宽计算发生在 uniform_list 的行构建闭包里，那个闭包拿不到 `&mut Window`，
    /// 所以在 render 里先缓存窗口宽度，闭包再读。
    last_pane_width: f32,
    new_folder_input: Entity<TextInput>,
    new_folder_open: bool,
    new_item_kind: NewItemKind,
    show_settings: bool,
    show_shortcuts: bool,
    left_preview_open: bool,
    right_preview_open: bool,
    favorites: Vec<PathBuf>,
    recents: Vec<PathBuf>,
    recent_limit: usize,
    shell_menu_default: bool,
    shell_menu_third_party: bool,
    rename_input: Entity<TextInput>,
    rename_open: bool,
    rename_target: Option<PathBuf>,
    /// 批量重命名：多选时的目标列表 + 模式输入
    batch_rename_open: bool,
    batch_rename_targets: Vec<PathBuf>,
    batch_rename_input: Entity<TextInput>,
    batch_rename_start: u32,
    context_menu_open: bool,
    context_menu_side: &'static str,
    context_menu_position: Option<Point<Pixels>>,
    context_menu_blank: bool,
    pending_context_target: Option<(&'static str, PathBuf)>,
    typeahead: String,
    typeahead_last_at: Option<Instant>,
    left_sort_column: SortColumn,
    left_sort_ascending: bool,
    right_sort_column: SortColumn,
    right_sort_ascending: bool,
    type_filter_open: bool,
    type_filter_side: &'static str,
    type_filter_allowed: HashSet<String>,
    left_load_generation: u64,
    right_load_generation: u64,
    watch_command_tx: mpsc::Sender<WatchCommand>,
    shortcut_bindings: BTreeMap<String, String>,
    shortcut_recording: Option<String>,
    /// Ctrl+Z 撤销栈（删除/重命名/新建），上限 32 条
    undo_stack: Vec<UndoEntry>,
}

impl FileFlowGpui {
    fn new(
        _window: &mut Window,
        watch_command_tx: mpsc::Sender<WatchCommand>,
        cx: &mut Context<Self>,
    ) -> Self {
        let fallback = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let saved = load_config(&fallback);
        let start = saved.left_path.clone();
        let entries = if is_virtual_path(&start) {
            virtual_entries(&start)
        } else {
            Vec::new()
        };
        let right_entries = if saved.split && is_virtual_path(&saved.right_path) {
            virtual_entries(&saved.right_path)
        } else {
            Vec::new()
        };
        let filter_input = cx.new(|cx| TextInput::new(cx, "输入名称筛选…"));
        let right_filter_input = cx.new(|cx| TextInput::new(cx, "输入名称筛选…"));
        let address_input = cx.new(|cx| TextInput::new(cx, "输入路径，例如 D:\\"));
        let new_folder_input = cx.new(|cx| {
            let mut input = TextInput::new(cx, "每行一个名称");
            input.set_multiline(true);
            input
        });
        let rename_input = cx.new(|cx| TextInput::new(cx, "新名称"));
        let batch_rename_input = cx.new(|cx| TextInput::new(cx, "批量名称（生成 名称_001、名称_002…）"));
        cx.observe(&filter_input, |this, _, cx| {
            this.status = "正在筛选左栏".to_string();
            cx.notify();
        })
        .detach();
        cx.observe(&right_filter_input, |this, _, cx| {
            this.status = "正在筛选右栏".to_string();
            cx.notify();
        })
        .detach();
        cx.observe(&address_input, |this, _, cx| {
            this.load_address_suggestions_async(cx);
        })
        .detach();
        Self {
            tabs: vec![start.clone()],
            active_tab: 0,
            left_back: Vec::new(),
            left_forward: Vec::new(),
            right_back: Vec::new(),
            right_forward: Vec::new(),
            active_side: "left",
            split: saved.split,
            right_path: saved.right_path.clone(),
            right_tabs: vec![saved.right_path],
            right_active_tab: 0,
            left_entries: entries.clone(),
            right_entries,
            left_display_entries: entries,
            right_display_entries: Vec::new(),
            left_view_mode: saved.view_mode,
            right_view_mode: saved.view_mode,
            status: "GPUI 迁移版：文件系统核心正在迁移".to_string(),
            folder_order: FolderOrder::FoldersFirst,
            show_hidden: saved.show_hidden,
            show_sidebar: saved.show_sidebar,
            focus_handle: cx.focus_handle(),
            left_scroll_handle: UniformListScrollHandle::new(),
            right_scroll_handle: UniformListScrollHandle::new(),
            left_selected: HashSet::new(),
            right_selected: HashSet::new(),
            left_anchor: None,
            right_anchor: None,
            left_focused: None,
            right_focused: None,
            drag_select: None,
            file_drag: None,
            suppress_blank_click: false,
            view_epoch: 0,
            filter_input,
            right_filter_input,
            address_input,
            address_suggestions: Vec::new(),
            address_suggestion_generation: 0,
            address_suggestions_suppress: false,
            address_editing: false,
            address_side: "left",
            ui_font_size: saved.ui_font_size,
            file_font_size: saved.file_font_size,
            row_spacing: saved.row_spacing.max(6.),
            load_thumbnails: saved.load_thumbnails,
            win_e_enabled: saved.win_e_enabled,
            explorer_replacement: saved.explorer_replacement,
            last_pane_width: 1280.0,
            new_folder_input,
            new_folder_open: false,
            new_item_kind: NewItemKind::Folder,
            show_settings: false,
            show_shortcuts: false,
            left_preview_open: false,
            right_preview_open: false,
            favorites: saved.favorites,
            recents: saved.recents,
            recent_limit: saved.recent_limit,
            shell_menu_default: saved.shell_menu_default,
            shell_menu_third_party: saved.shell_menu_third_party,
            rename_input,
            rename_open: false,
            rename_target: None,
            batch_rename_open: false,
            batch_rename_targets: Vec::new(),
            batch_rename_input,
            batch_rename_start: 1,
            context_menu_open: false,
            context_menu_side: "left",
            context_menu_position: None,
            context_menu_blank: false,
            pending_context_target: None,
            typeahead: String::new(),
            typeahead_last_at: None,
            left_sort_column: SortColumn::Name,
            left_sort_ascending: true,
            right_sort_column: SortColumn::Name,
            right_sort_ascending: true,
            type_filter_open: false,
            type_filter_side: "left",
            type_filter_allowed: HashSet::new(),
            left_load_generation: 0,
            right_load_generation: 0,
            watch_command_tx,
            shortcut_bindings: saved.shortcut_bindings,
            shortcut_recording: None,
            undo_stack: Vec::new(),
        }
    }

    fn current_path(&self) -> &Path {
        &self.tabs[self.active_tab]
    }

    fn sync_watched_paths(&self) {
        let mut paths = Vec::with_capacity(2);
        if !is_virtual_path(self.current_path()) {
            paths.push(self.current_path().to_path_buf());
        }
        if self.split && !is_virtual_path(&self.right_path) && !paths.contains(&self.right_path) {
            paths.push(self.right_path.clone());
        }
        let _ = self.watch_command_tx.send(WatchCommand::SetPaths(paths));
    }

    fn load_side_async(&mut self, side: &'static str, path: PathBuf, cx: &mut Context<Self>) {
        if is_virtual_path(&path) {
            let entries = virtual_entries(&path);
            if side == "right" {
                self.right_entries = entries;
            } else {
                self.left_entries = entries;
            }
            self.sync_watched_paths();
            cx.notify();
            return;
        }

        let generation = if side == "right" {
            self.right_load_generation = self.right_load_generation.wrapping_add(1);
            self.right_load_generation
        } else {
            self.left_load_generation = self.left_load_generation.wrapping_add(1);
            self.left_load_generation
        };
        self.status = format!("正在加载 {}", path.display());
        self.sync_watched_paths();
        cx.notify();

        let load_path = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move {
                let entries = read_entries(&load_path);
                // 网络共享目录打开成功：登记 \\\\server\\share，服务器根视图可回显
                if !entries.is_empty() || !load_path.starts_with("\\\\") {
                    remember_unc_share(&load_path);
                }
                entries
            });
        cx.spawn(async move |this, cx| {
            let entries = task.await;
            wait_out_of_ole_modal(cx).await;
            let _ = this.update(cx, |this, cx| {
                let still_current = if side == "right" {
                    this.right_load_generation == generation && this.right_path == path
                } else {
                    this.left_load_generation == generation && this.current_path() == path
                };
                if !still_current {
                    return;
                }
                let count = entries.len();
                if side == "right" {
                    this.right_entries = entries;
                } else {
                    this.left_entries = entries;
                }
                this.status = format!("{} 项", count);
                cx.notify();
            });
        })
        .detach();
    }

    fn reload_visible_async(&mut self, cx: &mut Context<Self>) {
        let left_path = self.current_path().to_path_buf();
        self.load_side_async("left", left_path, cx);
        if self.split {
            self.load_side_async("right", self.right_path.clone(), cx);
        }
    }

    fn load_address_suggestions_async(&mut self, cx: &mut Context<Self>) {
        let query = self.address_input.read(cx).value();
        // 点击地址栏进入编辑时的 set_value 也会触发 observe——跳过这轮探测，
        // 避免每次点地址栏都对当前路径（可能是网络盘）跑一轮 I/O
        if self.address_suggestions_suppress {
            self.address_suggestions_suppress = false;
            self.address_suggestion_generation =
                self.address_suggestion_generation.wrapping_add(1);
            return;
        }
        self.address_suggestion_generation = self.address_suggestion_generation.wrapping_add(1);
        let generation = self.address_suggestion_generation;
        let task = cx
            .background_executor()
            .spawn(async move { address_suggestions(&query) });
        cx.spawn(async move |this, cx| {
            let suggestions = task.await;
            wait_out_of_ole_modal(cx).await;
            let _ = this.update(cx, |this, cx| {
                if this.address_suggestion_generation != generation {
                    return;
                }
                this.address_suggestions = suggestions;
                cx.notify();
            });
        })
        .detach();
    }

    fn remember_recent(&mut self, path: &Path) {
        if is_virtual_path(path) || is_network_path(path) || is_recycle_bin_path(path) {
            return;
        }
        if !path.exists() && !can_open_directory(path) {
            return;
        }
        self.recents.retain(|item| item != path);
        if self.recent_limit > 0 {
            self.recents.insert(0, path.to_path_buf());
        }
        self.recents.truncate(self.recent_limit);
    }

    fn navigate_left(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = normalize_path(path);
        if can_open_directory(&path) {
            self.remember_recent(&path);
            let current = self.current_path().to_path_buf();
            if current != path {
                if current.starts_with(&path) {
                    self.left_forward.push(current);
                } else {
                    self.left_back.push(current);
                    self.left_forward.clear();
                }
            }
            self.tabs[self.active_tab] = path.clone();
            self.left_selected.clear();
            self.clear_side_filter("left", cx);
            self.load_side_async("left", path, cx);
            self.save_config();
            cx.notify();
        }
    }

    fn open_virtual(&mut self, side: &'static str, virtual_path: PathBuf, cx: &mut Context<Self>) {
        let entries = virtual_entries(&virtual_path);
        if side == "left" {
            let current = self.current_path().to_path_buf();
            if current != virtual_path {
                self.left_back.push(current);
                self.left_forward.clear();
            }
            self.tabs[self.active_tab] = virtual_path.clone();
            self.left_entries = entries;
            self.left_selected.clear();
            self.clear_side_filter("left", cx);
        } else {
            if self.right_path != virtual_path {
                self.right_back.push(self.right_path.clone());
                self.right_forward.clear();
            }
            self.right_path = virtual_path.clone();
            self.right_tabs[self.right_active_tab] = virtual_path.clone();
            self.right_entries = entries;
            self.right_selected.clear();
            self.clear_side_filter("right", cx);
        }
        self.active_side = side;
        self.status = virtual_title(&virtual_path).to_string();
        self.sync_watched_paths();
        cx.notify();
    }

    fn navigate_right(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = normalize_path(path);
        if can_open_directory(&path) {
            self.remember_recent(&path);
            let current = self.right_path.clone();
            if current != path {
                if current.starts_with(&path) {
                    self.right_forward.push(current);
                } else {
                    self.right_back.push(current);
                    self.right_forward.clear();
                }
            }
            self.right_path = path.clone();
            self.right_tabs[self.right_active_tab] = self.right_path.clone();
            self.right_selected.clear();
            self.clear_side_filter("right", cx);
            self.load_side_async("right", path, cx);
            self.save_config();
            cx.notify();
        }
    }

    fn navigate_active(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.split && self.active_side == "right" {
            self.navigate_right(path, cx);
        } else {
            self.navigate_left(path, cx);
        }
    }

    fn open_external_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = normalize_path(path);
        let (folder, selected) = if path.is_file() {
            (
                path.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| path.clone()),
                Some(path),
            )
        } else {
            (path.clone(), None)
        };
        if !can_open_directory(&folder) {
            self.status = format!("无法打开外部路径：{}", folder.display());
            cx.notify();
            return;
        }
        self.active_side = "left";
        self.navigate_left(folder.clone(), cx);
        if let Some(selected) = selected {
            self.left_selected.clear();
            self.left_selected.insert(selected.clone());
            self.left_anchor = Some(selected.clone());
            self.left_focused = Some(selected);
        }
        self.status = format!("已从外部打开：{}", folder.display());
        cx.notify();
    }

    fn open_virtual_active(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let side = if self.split && self.active_side == "right" {
            "right"
        } else {
            "left"
        };
        self.open_virtual(side, path, cx);
    }

    fn navigate_left_history(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = normalize_path(path);
        self.clear_side_filter("left", cx);
        if is_virtual_path(&path) {
            self.tabs[self.active_tab] = path;
            self.left_entries = virtual_entries(self.current_path());
            cx.notify();
        } else if path.is_dir() {
            self.tabs[self.active_tab] = path.clone();
            self.load_side_async("left", path, cx);
            self.save_config();
            cx.notify();
        }
    }
    fn navigate_right_history(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path = normalize_path(path);
        self.clear_side_filter("right", cx);
        if is_virtual_path(&path) {
            self.right_path = path;
            self.right_tabs[self.right_active_tab] = self.right_path.clone();
            self.right_entries = virtual_entries(&self.right_path);
            cx.notify();
        } else if path.is_dir() {
            self.right_path = path.clone();
            self.right_tabs[self.right_active_tab] = self.right_path.clone();
            self.load_side_async("right", path, cx);
            self.save_config();
            cx.notify();
        }
    }
    fn go_up_side(&mut self, side: &'static str, cx: &mut Context<Self>) {
        // 记住出发目录：返回上级后选中它（资源管理器行为——刚离开的
        // 子文件夹呈选中态，方便再 Enter 回去或对其操作）
        let origin = if side == "left" {
            self.current_path().to_path_buf()
        } else {
            self.right_path.clone()
        };
        let parent = origin.parent().map(|parent| parent.to_path_buf());
        if let Some(parent) = parent {
            if side == "left" {
                self.navigate_left(parent, cx);
            } else {
                self.navigate_right(parent, cx);
            }
            if !is_virtual_path(&origin) && origin.file_name().is_some() {
                self.set_single_selection(side, origin);
            }
        }
    }
    fn go_back(&mut self, side: &'static str, cx: &mut Context<Self>) {
        if side == "left" {
            if let Some(path) = self.left_back.pop() {
                self.left_forward.push(self.current_path().to_path_buf());
                self.navigate_left_history(path, cx);
            }
        } else if let Some(path) = self.right_back.pop() {
            self.right_forward.push(self.right_path.clone());
            self.navigate_right_history(path, cx);
        }
    }
    fn go_forward(&mut self, side: &'static str, cx: &mut Context<Self>) {
        if side == "left" {
            if let Some(path) = self.left_forward.pop() {
                self.left_back.push(self.current_path().to_path_buf());
                self.navigate_left_history(path, cx);
            }
        } else if let Some(path) = self.right_forward.pop() {
            self.right_back.push(self.right_path.clone());
            self.navigate_right_history(path, cx);
        }
    }

    fn add_tab(&mut self, cx: &mut Context<Self>) {
        let path = self.current_path().to_path_buf();
        self.tabs.push(path);
        self.active_tab = self.tabs.len() - 1;
        self.status = "已新建标签页".to_string();
        cx.notify();
    }

    fn add_right_tab(&mut self, cx: &mut Context<Self>) {
        self.right_tabs.push(self.right_path.clone());
        self.right_active_tab = self.right_tabs.len() - 1;
        self.load_side_async("right", self.right_path.clone(), cx);
        cx.notify();
    }

    fn close_right_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.right_tabs.len() == 1 {
            return;
        }
        self.right_tabs.remove(index);
        self.right_active_tab = self.right_active_tab.min(self.right_tabs.len() - 1);
        self.right_path = self.right_tabs[self.right_active_tab].clone();
        self.load_side_async("right", self.right_path.clone(), cx);
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() == 1 {
            return;
        }
        self.tabs.remove(index);
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let path = self.current_path().to_path_buf();
        self.load_side_async("left", path, cx);
        cx.notify();
    }

    fn toggle_split(&mut self, cx: &mut Context<Self>) {
        self.split = !self.split;
        if self.split {
            self.right_path = self.current_path().to_path_buf();
            self.right_tabs[self.right_active_tab] = self.right_path.clone();
            self.load_side_async("right", self.right_path.clone(), cx);
        } else {
            self.sync_watched_paths();
        }
        self.save_config();
        cx.notify();
    }

    fn view_mode_for(&self, side: &'static str) -> ViewMode {
        if side == "right" {
            self.right_view_mode
        } else {
            self.left_view_mode
        }
    }

    fn set_view_mode_for(&mut self, side: &'static str, view_mode: ViewMode) {
        if side == "right" {
            self.right_view_mode = view_mode;
        } else {
            self.left_view_mode = view_mode;
        }
    }

    fn cycle_view(&mut self, direction: i32, cx: &mut Context<Self>) {
        let modes = [
            ViewMode::Details,
            ViewMode::List,
            ViewMode::Columns,
            ViewMode::MIcons,
            ViewMode::LIcons,
            ViewMode::XLIcons,
        ];
        let current = self.view_mode_for(self.active_side);
        let index = modes.iter().position(|mode| *mode == current).unwrap_or(0) as i32;
        let next = modes[(index + direction).clamp(0, modes.len() as i32 - 1) as usize];
        if next == current {
            return;
        }
        self.set_view_mode_for(self.active_side, next);
        self.view_epoch = self.view_epoch.wrapping_add(1);
        self.status = format!("查看方式：{}", next.label());
        self.save_config();
        cx.notify();
    }
    fn set_view(&mut self, view_mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode_for(self.active_side) == view_mode {
            return;
        }
        self.set_view_mode_for(self.active_side, view_mode);
        self.view_epoch = self.view_epoch.wrapping_add(1);
        self.status = format!("查看方式：{}", view_mode.label());
        self.save_config();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload_visible_async(cx);
    }

    fn cycle_folders(&mut self, cx: &mut Context<Self>) {
        self.folder_order = self.folder_order.next();
        self.status = self.folder_order.label().to_string();
        cx.notify();
    }

    fn dismiss_popups_except_new_folder(&mut self) -> bool {
        let changed = self.context_menu_open
            || self.show_settings
            || self.show_shortcuts
            || self.address_editing
            || self.rename_open
            || self.batch_rename_open
            || self.type_filter_open;
        self.context_menu_open = false;
        self.show_settings = false;
        self.show_shortcuts = false;
        self.address_editing = false;
        self.rename_open = false;
        self.rename_target = None;
        self.batch_rename_open = false;
        self.batch_rename_targets.clear();
        self.type_filter_open = false;
        changed
    }

    fn sort_state_for(&self, side: &'static str) -> (SortColumn, bool) {
        if side == "right" {
            (self.right_sort_column, self.right_sort_ascending)
        } else {
            (self.left_sort_column, self.left_sort_ascending)
        }
    }

    /// Details 三列宽度 [类型, 日期, 大小]（逻辑像素）——**列宽单一来源**。
    ///
    /// 表头、数据行、空白单元格都消费这里返回值，保证三列 x 坐标恒定。
    ///
    /// 策略：**先给名称列留出最小宽度，剩余空间按内容占比分给三列**。
    /// 原来是固定 112/158/90（只看是否分栏乘 0.55，不看窗口宽度），
    /// 窗口一宽名称列就独吞全部剩余空间、三列显得局促。
    ///
    /// 占比按内容需要定：日期最占（完整时间戳 "2026/10/3 03:42"），
    /// 类型次之（要放"文件夹 ▾"和筛选箭头），大小最小（"1.2 MB"）。
    fn details_col_widths(&self) -> [f32; 3] {
        /// 名称列左侧固定占用：缩进 30（图标位）+ 图标 15 + gap 12
        const NAME_CHROME: f32 = 57.0;
        /// 名称列必须保留的最小宽度（放得下常见中文文件名）
        const NAME_MIN: f32 = 190.0;
        /// 三列合计的最大占比（剩下的留给名称列，避免三列喧宾夺主）
        const COL_MAX_RATIO: f32 = 0.52;
        /// 三列之间共 2 个 12px 间隙
        const GAPS: f32 = 24.0;

        let pane = self.last_pane_width.max(280.0);
        // 分栏时每侧只有一半宽
        let pane = if self.split { pane * 0.5 } else { pane };

        // 名称列的绝对底线：无论窗口多窄，三列都必须让出这么多，
        // 否则窄窗口 + 分栏时三列会把名称挤到放不下一个字。
        let col_total_max = (pane - NAME_CHROME - NAME_MIN - GAPS).max(96.0);
        // 富余空间按占比分配
        let col_budget = (col_total_max * COL_MAX_RATIO).max(96.0);
        let total = col_budget - GAPS;

        const SHARE_TYPE: f32 = 0.30;
        const SHARE_DATE: f32 = 0.44;
        const SHARE_SIZE: f32 = 0.26;
        let type_w = (total * SHARE_TYPE).clamp(72.0, 260.0);
        let date_w = (total * SHARE_DATE).clamp(96.0, 320.0);
        let size_w = (total * SHARE_SIZE).clamp(64.0, 220.0);

        // clamp 可能突破上限（极窄窗口），按比例回收，保证名称列不被打穿。
        // 回收时给每列设 56px 地板——再窄也要能显示 "123 MB"，
        // 否则窄分栏下"大小"列会缩成一个数字都放不下的窄条。
        let sum = type_w + date_w + size_w;
        if sum > col_total_max && sum > 0.0 {
            let k = col_total_max / sum;
            return [
                (type_w * k).round().max(56.0),
                (date_w * k).round().max(56.0),
                (size_w * k).round().max(56.0),
            ];
        }
        [type_w.round(), date_w.round(), size_w.round()]
    }

    fn arranged_entries(&self, entries: &[Entry], query: &str, side: &'static str) -> Vec<Entry> {
        let query = query.to_lowercase();
        let (sort_column, sort_ascending) = self.sort_state_for(side);
        let mut result: Vec<Entry> = entries
            .iter()
            .filter(|entry| self.show_hidden || !entry.name.starts_with('.'))
            .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .filter(|entry| {
                self.type_filter_allowed.is_empty()
                    || self.type_filter_allowed.contains(&type_label(entry))
            })
            .cloned()
            .collect();
        match self.folder_order {
            FolderOrder::FoldersOnly => result.retain(|entry| entry.is_dir),
            FolderOrder::Mixed => result.sort_by_key(|entry| entry.name.to_lowercase()),
            FolderOrder::FoldersFirst => result.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
            FolderOrder::FilesFirst => result.sort_by(|a, b| {
                a.is_dir
                    .cmp(&b.is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
        }
        result.sort_by(|a, b| {
            let folder_group = match self.folder_order {
                FolderOrder::FoldersFirst => b.is_dir.cmp(&a.is_dir),
                FolderOrder::FilesFirst => a.is_dir.cmp(&b.is_dir),
                _ => Ordering::Equal,
            };
            let column_order = compare_entries(a, b, sort_column);
            folder_group.then_with(|| {
                if sort_ascending {
                    column_order
                } else {
                    column_order.reverse()
                }
            })
        });
        result
    }

    /// 按栏取筛选输入：每栏独立筛选，焦点在哪个栏 Ctrl+F 就搜哪栏
    fn filter_input_for(&self, side: &'static str) -> &Entity<TextInput> {
        if side == "right" {
            &self.right_filter_input
        } else {
            &self.filter_input
        }
    }

    /// 清空该栏的搜索/筛选（切换目录时自动取消搜索）
    fn clear_side_filter(&mut self, side: &'static str, cx: &mut Context<Self>) {
        let input = self.filter_input_for(side).clone();
        input.update(cx, |input, _| {
            if !input.value().is_empty() {
                input.clear();
            }
        });
    }

    /// 按路径找已加载条目（渲染用：is_dir/size 直接取 Entry，零磁盘 IO）
    fn entry_of(&self, path: &Path) -> Option<&Entry> {
        self.left_entries
            .iter()
            .chain(self.right_entries.iter())
            .find(|entry| entry.path == path)
    }

    /// 排序切换（底栏按钮/合并菜单共用）：按 名称→类型→修改日期→大小 循环，再点同列翻转升降序
    fn cycle_sort(&mut self, cx: &mut Context<Self>) {
        let side = self.active_side;
        integration_log(&format!("cycle_sort: side={side}"));
        let (current, _) = self.sort_state_for(side);
        let next = match current {
            SortColumn::Name => SortColumn::Type,
            SortColumn::Type => SortColumn::Modified,
            SortColumn::Modified => SortColumn::Size,
            SortColumn::Size => SortColumn::Name,
        };
        self.sort_by(side, next, cx);
    }

    /// 显式设置排序方向（不改列），底部栏 Ctrl+点击用
    fn set_sort_ascending(&mut self, side: &'static str, ascending: bool, cx: &mut Context<Self>) {
        if side == "right" {
            self.right_sort_ascending = ascending;
        } else {
            self.left_sort_ascending = ascending;
        }
        cx.notify();
    }

    fn sort_by(&mut self, side: &'static str, column: SortColumn, cx: &mut Context<Self>) {
        self.active_side = side;
        let ascending = if side == "right" {            if self.right_sort_column == column {
                self.right_sort_ascending = !self.right_sort_ascending;
            } else {
                self.right_sort_column = column;
                self.right_sort_ascending = true;
            }
            self.right_sort_ascending
        } else {
            if self.left_sort_column == column {
                self.left_sort_ascending = !self.left_sort_ascending;
            } else {
                self.left_sort_column = column;
                self.left_sort_ascending = true;
            }
            self.left_sort_ascending
        };
        self.status = format!(
            "{}栏排序：{} {}",
            if side == "right" { "右" } else { "左" },
            sort_column_label(column),
            if ascending { "升序" } else { "降序" }
        );
        cx.notify();
    }

    fn open_type_filter(&mut self, side: &'static str, cx: &mut Context<Self>) {
        let should_open = !self.type_filter_open || self.type_filter_side != side;
        self.type_filter_side = side;
        self.type_filter_open = should_open;
        self.active_side = side;
        cx.notify();
    }

    fn toggle_type_filter(&mut self, label: String, cx: &mut Context<Self>) {
        let entries = if self.type_filter_side == "right" {
            &self.right_entries
        } else {
            &self.left_entries
        };
        let all: HashSet<String> = type_counts(entries).into_keys().collect();
        let all_len = all.len();
        if self.type_filter_allowed.is_empty() {
            self.type_filter_allowed = all;
        }
        if self.type_filter_allowed.contains(&label) {
            self.type_filter_allowed.remove(&label);
        } else {
            self.type_filter_allowed.insert(label);
        }
        if self.type_filter_allowed.len() == all_len {
            self.type_filter_allowed.clear();
        }
        cx.notify();
    }

    fn select_by_prefix(&mut self, typed: &str, cx: &mut Context<Self>) {
        const TYPEAHEAD_TIMEOUT: Duration = Duration::from_millis(900);

        let now = Instant::now();
        let still_typing = self
            .typeahead_last_at
            .is_some_and(|last| now.saturating_duration_since(last) <= TYPEAHEAD_TIMEOUT);
        let cycle_same_initial = still_typing
            && self.typeahead.chars().count() == 1
            && self.typeahead.eq_ignore_ascii_case(typed);
        let mut prefix = if still_typing && !cycle_same_initial {
            format!("{}{}", self.typeahead, typed)
        } else {
            typed.to_string()
        };

        let query = self
            .filter_input_for(self.active_side)
            .read(cx)
            .value();
        let entries = if self.active_side == "left" {
            self.arranged_entries(&self.left_entries, &query, "left")
        } else {
            self.arranged_entries(&self.right_entries, &query, "right")
        };

        let matching_folders = |value: &str| {
            let lower = value.to_lowercase();
            entries
                .iter()
                .filter(|entry| entry.is_dir && entry.name.to_lowercase().starts_with(&lower))
                .map(|entry| (entry.path.clone(), entry.name.clone()))
                .collect::<Vec<_>>()
        };
        let mut matches = matching_folders(&prefix);

        // If a new character makes the accumulated prefix invalid, start a fresh search
        // with that character so typing never leaves the selection feeling stuck.
        if matches.is_empty() && prefix.chars().count() > 1 {
            prefix = typed.to_string();
            matches = matching_folders(&prefix);
        }

        self.typeahead = prefix;
        self.typeahead_last_at = Some(now);
        if matches.is_empty() {
            return;
        }

        let focused = if self.active_side == "left" {
            self.left_focused.as_ref()
        } else {
            self.right_focused.as_ref()
        };
        let target_index = if cycle_same_initial {
            focused
                .and_then(|path| matches.iter().position(|(candidate, _)| candidate == path))
                .map(|index| (index + 1) % matches.len())
                .unwrap_or(0)
        } else {
            0
        };
        let (path, name) = matches[target_index].clone();
        let arranged_index = entries
            .iter()
            .position(|entry| entry.path == path)
            .unwrap_or(target_index);
        let view_mode = self.view_mode_for(self.active_side);
        let scroll_index = if matches!(
            view_mode,
            ViewMode::Columns | ViewMode::MIcons | ViewMode::LIcons | ViewMode::XLIcons
        ) {
            arranged_index
        } else {
            arranged_index + 1
        };
        if self.active_side == "left" {
            self.left_scroll_handle.scroll_to_item(scroll_index, ScrollStrategy::Top);
        } else {
            self.right_scroll_handle.scroll_to_item(scroll_index, ScrollStrategy::Top);
        }

        if self.active_side == "left" {
            self.left_selected.clear();
            self.left_selected.insert(path.clone());
            self.left_anchor = Some(path.clone());
            self.left_focused = Some(path);
        } else {
            self.right_selected.clear();
            self.right_selected.insert(path.clone());
            self.right_anchor = Some(path.clone());
            self.right_focused = Some(path);
        }
        self.status = format!("已定位文件夹：{}", name);
        cx.notify();
    }

    fn is_selected(&self, side: &'static str, path: &Path) -> bool {
        if side == "left" {
            self.left_selected.contains(path)
        } else {
            self.right_selected.contains(path)
        }
    }

    /// 方向键移动选中：网格视图按列数换算，列表/详情一行一步。
    /// step: (行变化, 列变化)。焦点在文本输入框时方向键属于编辑操作，跳过。
    fn navigate_selection(&mut self, row_step: i32, col_step: i32, window: &mut Window, cx: &mut Context<Self>) {
        // 焦点在输入框（搜索/地址/重命名/批量重命名/新建）时不抢方向键
        let inputs = [
            &self.filter_input,
            &self.right_filter_input,
            &self.address_input,
            &self.new_folder_input,
            &self.rename_input,
            &self.batch_rename_input,
        ];
        let typing = self.rename_open
            || self.batch_rename_open
            || self.new_folder_open
            || self.address_editing
            || inputs
                .iter()
                .any(|input| input.read(cx).focus_handle(cx).is_focused(window));
        if typing {
            return;
        }
        let side = self.active_side;
        let query = self.filter_input_for(side).read(cx).value();
        let entries = if side == "left" {
            self.arranged_entries(&self.left_entries, &query, "left")
        } else {
            self.arranged_entries(&self.right_entries, &query, "right")
        };
        if entries.is_empty() {
            return;
        }
        let focused = if side == "left" {
            self.left_focused.as_deref()
        } else {
            self.right_focused.as_deref()
        };
        let current = focused
            .and_then(|path| entries.iter().position(|entry| entry.path == path))
            .unwrap_or(0);
        let view_mode = self.view_mode_for(side);
        let (row, column, columns) = match view_mode {
            ViewMode::Columns | ViewMode::MIcons | ViewMode::LIcons | ViewMode::XLIcons => {
                // 网格：列数未知（视窗宽度决定），用启发式——图标的实际列宽
                let columns = self.grid_columns_for(side, view_mode).max(1);
                (current / columns, current % columns, columns)
            }
            ViewMode::Details | ViewMode::List => (current, 0, 1),
        };
        let total = entries.len() as i32;
        let next = if view_mode == ViewMode::Details || view_mode == ViewMode::List {
            (row as i32 + row_step + col_step).clamp(0, total - 1) as usize
        } else {
            let next_row = (row as i32 + row_step).clamp(0, (total - 1) / columns as i32);
            let next_column = (column as i32 + col_step).clamp(0, columns as i32 - 1);
            let candidate = next_row * columns as i32 + next_column;
            candidate.clamp(0, total - 1) as usize
        };
        if next == current && focused.is_some() {
            return;
        }
        let path = entries[next].path.clone();
        self.set_single_selection(side, path.clone());
        // 滚动到可见（列表视图首行是表头 +1）
        let scroll_index = if matches!(
            view_mode,
            ViewMode::Columns | ViewMode::MIcons | ViewMode::LIcons | ViewMode::XLIcons
        ) {
            next
        } else {
            next + 1
        };
        if side == "left" {
            self.left_scroll_handle.scroll_to_item(scroll_index, ScrollStrategy::Top);
        } else {
            self.right_scroll_handle.scroll_to_item(scroll_index, ScrollStrategy::Top);
        }
        self.status = entries[next].name.clone();
        cx.notify();
    }

    /// 网格视图当前列数（虚拟化后渲染、拖框、拖放、键盘导航统一走
    /// virtual_grid_geometry；此处保留给旧调用点，内部同源）
    fn grid_columns_for(&self, side: &'static str, view_mode: ViewMode) -> usize {
        let handle = if side == "left" {
            self.left_scroll_handle.0.borrow().base_handle.clone()
        } else {
            self.right_scroll_handle.0.borrow().base_handle.clone()
        };
        let viewport_width = f32::from(handle.bounds().size.width).max(1.);
        let geo = virtual_grid_geometry(
            view_mode,
            self.file_font_size,
            self.row_spacing,
            viewport_width,
            usize::MAX,
        );
        geo.cols.max(1)
    }

    fn is_focused_selection(&self, side: &'static str, path: &Path) -> bool {
        if side == "left" {
            self.left_focused.as_deref() == Some(path)
        } else {
            self.right_focused.as_deref() == Some(path)
        }
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        let selected = if self.active_side == "left" {
            &self.left_selected
        } else {
            &self.right_selected
        };
        selected.iter().cloned().collect()
    }

    fn selected_path(&self) -> Option<PathBuf> {
        self.selected_paths().into_iter().next()
    }

    fn set_single_selection(&mut self, side: &'static str, path: PathBuf) {
        self.active_side = side;
        if side == "left" {
            self.left_selected.clear();
            self.left_selected.insert(path.clone());
            self.left_anchor = Some(path);
            self.left_focused = self.left_anchor.clone();
        } else {
            self.right_selected.clear();
            self.right_selected.insert(path.clone());
            self.right_anchor = Some(path);
            self.right_focused = self.right_anchor.clone();
        }
    }

    fn clear_selection(&mut self, side: &'static str) {
        if side == "left" {
            self.left_selected.clear();
            self.left_focused = None;
        } else {
            self.right_selected.clear();
            self.right_focused = None;
        }
    }

    fn select_entry(
        &mut self,
        side: &'static str,
        path: PathBuf,
        visible_paths: &[PathBuf],
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;

        if modifiers.shift {
            let anchor = (if side == "left" {
                self.left_anchor.clone()
            } else {
                self.right_anchor.clone()
            })
            .unwrap_or_else(|| path.clone());
            let anchor_index = visible_paths.iter().position(|item| item == &anchor);
            let current_index = visible_paths.iter().position(|item| item == &path);
            if let (Some(a), Some(b)) = (anchor_index, current_index) {
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                let range: HashSet<PathBuf> = visible_paths[start..=end].iter().cloned().collect();
                if side == "left" {
                    self.left_selected = range;
                } else {
                    self.right_selected = range;
                }
                if side == "left" {
                    self.left_focused = Some(path.clone());
                } else {
                    self.right_focused = Some(path.clone());
                }
            } else {
                self.set_single_selection(side, path.clone());
            }
        } else if modifiers.control {
            let selected = if side == "left" {
                &mut self.left_selected
            } else {
                &mut self.right_selected
            };
            if !selected.remove(&path) {
                selected.insert(path.clone());
            }
            if side == "left" {
                self.left_anchor = Some(path.clone());
                self.left_focused = Some(path);
            } else {
                self.right_anchor = Some(path.clone());
                self.right_focused = Some(path);
            }
        } else {
            self.set_single_selection(side, path);
        }
        let count = if side == "left" {
            self.left_selected.len()
        } else {
            self.right_selected.len()
        };
        self.status = format!("已选择 {count} 项");
        cx.notify();
    }

    fn on_blank_area_click(
        &mut self,
        side: &'static str,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        if self.context_menu_open && !event.is_right_click() {
            self.context_menu_open = false;
            self.suppress_blank_click = false;
            cx.notify();
            return;
        }
        if self.suppress_blank_click {
            self.suppress_blank_click = false;
            cx.notify();
            return;
        }
        // gpui 的 on_click 只对左键触发（pending_mouse_down 仅记录 Left），
        // 右键菜单统一走 mouse_down，这里不再有右键分支（否则会弹出第二个菜单）
        if self.dismiss_popups_except_new_folder() {
            cx.notify();
        } else if event.click_count() >= 2 {
            self.go_up_side(side, cx);
        } else {
            self.clear_selection(side);
            self.status = "已取消选择".to_string();
            cx.notify();
        }
    }

    fn begin_drag_select(
        &mut self,
        side: &'static str,
        position: Point<Pixels>,
        visible_paths: Vec<PathBuf>,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        self.dismiss_popups_except_new_folder();
        self.drag_select = Some(DragSelectState {
            side,
            start: position,
            current: position,
            visible_paths,
            modifiers,
            active: false,
        });
        cx.notify();
    }

    fn cancel_drag_select(&mut self) {
        self.drag_select = None;
    }

    fn begin_file_drag(
        &mut self,
        side: &'static str,
        path: PathBuf,
        visible_paths: &[PathBuf],
        position: Point<Pixels>,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        self.dismiss_popups_except_new_folder();
        if !self.is_selected(side, &path) {
            self.select_entry(side, path.clone(), visible_paths, modifiers, cx);
        }
        let sources = if side == "left" {
            self.left_selected.iter().cloned().collect()
        } else {
            self.right_selected.iter().cloned().collect()
        };
        self.file_drag = Some(FileDragState {
            start: position,
            current: position,
            sources,
            modifiers,
            active: false,
            external_started: false,
        });
        self.drag_select = None;
        cx.notify();
    }

    fn update_file_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.file_drag = None;
            cx.stop_active_drag(window);
            return;
        }
        let mut external_sources = None;
        let Some(state) = self.file_drag.as_mut() else {
            return;
        };
        state.current = event.position;
        let dx = (pixel_value(state.current.x) - pixel_value(state.start.x)).abs();
        let dy = (pixel_value(state.current.y) - pixel_value(state.start.y)).abs();
        let distance = (dx * dx + dy * dy).sqrt();
        if distance >= 8.0 {
            state.active = true;
            if !state.external_started {
                state.external_started = true;
                external_sources = Some(state.sources.clone());
            }
        }
        if let Some(sources) = external_sources {
            self.status = format!("正在拖出 {} 项", sources.len());
            self.file_drag = None;
            cx.stop_active_drag(window);
            unsafe {
                let _ = ReleaseCapture();
            }
            cx.notify();
            let ok = start_windows_file_drag(&sources);
            self.status = if ok {
                format!("已把 {} 项交给 Windows 拖拽", sources.len())
            } else {
                "外部拖拽没有被目标应用接收".to_string()
            };
            self.suppress_blank_click = true;
            cx.notify();
        }
    }

    fn finish_file_drag(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        cx.stop_active_drag(window);
        let Some(state) = self.file_drag.take() else {
            return false;
        };
        if !state.active {
            return false;
        }
        let target_dir = self.file_drop_target(event.position, window);
        let Some(target_dir) = target_dir else {
            self.status = "未找到可放置的位置".to_string();
            self.suppress_blank_click = true;
            cx.notify();
            return true;
        };
        if is_virtual_path(&target_dir) || !target_dir.is_dir() {
            self.status = "只能拖放到文件夹".to_string();
            self.suppress_blank_click = true;
            cx.notify();
            return true;
        }
        let copy = state.modifiers.control;
        self.drop_files_to(&state.sources, &target_dir, copy, cx);
        self.suppress_blank_click = true;
        true
    }

    fn file_drop_target(&self, position: Point<Pixels>, _window: &Window) -> Option<PathBuf> {
        let point = (pixel_value(position.x), pixel_value(position.y));
        let mut best: Option<(f32, &PathBuf)> = None;
        for (side, entries) in [("left", &self.left_entries), ("right", &self.right_entries)] {
            if side == "right" && !self.split {
                continue;
            }
            // ScrollHandle 真实绘制 bounds + 滚动偏移（entry_bounds 在 track_scroll
            // 容器上恒为空，不可用）
            let handle = if side == "left" {
                self.left_scroll_handle.0.borrow().base_handle.clone()
            } else {
                self.right_scroll_handle.0.borrow().base_handle.clone()
            };
            let offset = handle.offset();
            for (index, entry) in entries.iter().enumerate() {
                if !entry.is_dir {
                    continue;
                }
                let Some(bounds) = handle.bounds_for_item(index) else {
                    continue;
                };
                let rect = (
                    pixel_value(bounds.left()) + pixel_value(offset.x),
                    pixel_value(bounds.top()) + pixel_value(offset.y),
                    pixel_value(bounds.right()) + pixel_value(offset.x),
                    pixel_value(bounds.bottom()) + pixel_value(offset.y),
                );
                if point_in_rect(point, rect) {
                    // 多个嵌套命中时取面积最小（最内层）的
                    let area = (rect.2 - rect.0) * (rect.3 - rect.1);
                    if best.as_ref().is_none_or(|(a, _)| area < *a) {
                        best = Some((area, &entry.path));
                    }
                }
            }
        }
        if let Some((_, path)) = best {
            return Some(path.clone());
        }
        let side = self.side_at_position(position, _window);
        Some(if side == "right" && self.split {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        })
    }

    fn side_at_position(&self, position: Point<Pixels>, window: &Window) -> &'static str {
        if !self.split {
            return "left";
        }
        let window_width = pixel_value(window.bounds().size.width);
        let sidebar_w = if self.show_sidebar { 218.0 } else { 0.0 };
        let pane_area_w = (window_width - sidebar_w).max(1.0);
        let middle = sidebar_w + pane_area_w / 2.0;
        if pixel_value(position.x) >= middle {
            "right"
        } else {
            "left"
        }
    }

    fn update_drag_select(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.drag_select = None;
            self.suppress_blank_click = false;
            return;
        }
        let Some((side, start, current, visible_paths, modifiers, active)) =
            self.drag_select.as_mut().map(|state| {
                state.current = event.position;
                let dx = (pixel_value(state.current.x) - pixel_value(state.start.x)).abs();
                let dy = (pixel_value(state.current.y) - pixel_value(state.start.y)).abs();
                let distance = (dx * dx + dy * dy).sqrt();
                if distance >= 24.0 && (dx >= 12.0 || dy >= 12.0) {
                    state.active = true;
                }
                (
                    state.side,
                    state.start,
                    state.current,
                    state.visible_paths.clone(),
                    state.modifiers,
                    state.active,
                )
            })
        else {
            return;
        };
        if !active {
            return;
        }
        self.apply_drag_selection(side, start, current, &visible_paths, modifiers, window);
        cx.notify();
    }

    fn apply_drag_selection(
        &mut self,
        side: &'static str,
        start: Point<Pixels>,
        current: Point<Pixels>,
        visible_paths: &[PathBuf],
        modifiers: gpui::Modifiers,
        _window: &mut Window,
    ) {
        let x1 = pixel_value(start.x).min(pixel_value(current.x));
        let x2 = pixel_value(start.x).max(pixel_value(current.x));
        let y1 = pixel_value(start.y).min(pixel_value(current.y));
        let y2 = pixel_value(start.y).max(pixel_value(current.y));
        let selection = (x1, y1, x2, y2);
        // 从 ScrollHandle 取每个条目的真实绘制 bounds（on_children_prepainted 在
        // track_scroll 容器上拿不到子元素 bounds，镜像计算会因 gap/自适应行高错位）
        let scroll_handle = if side == "left" {
            self.left_scroll_handle.0.borrow().base_handle.clone()
        } else {
            self.right_scroll_handle.0.borrow().base_handle.clone()
        };
        let scroll_offset = scroll_handle.offset();
        let mut range = HashSet::new();
        let mut focused = None;
        let mut matched = false;
        for (index, path) in visible_paths.iter().enumerate() {
            let Some(bounds) = scroll_handle.bounds_for_item(index) else {
                continue;
            };
            matched = true;
            // child_bounds 是内容坐标，鼠标事件是窗口坐标：加回滚动偏移
            let item = (
                pixel_value(bounds.left()) + pixel_value(scroll_offset.x),
                pixel_value(bounds.top()) + pixel_value(scroll_offset.y),
                pixel_value(bounds.right()) + pixel_value(scroll_offset.x),
                pixel_value(bounds.bottom()) + pixel_value(scroll_offset.y),
            );
            if rect_intersects(selection, item) {
                range.insert(path.clone());
                focused = Some(path.clone());
            }
        }
        if matched {
            if side == "left" {
                if modifiers.control {
                    self.left_selected.extend(range);
                } else {
                    self.left_selected = range;
                }
                self.left_focused = focused;
            } else {
                if modifiers.control {
                    self.right_selected.extend(range);
                } else {
                    self.right_selected = range;
                }
                self.right_focused = focused;
            }
            return;
        }
        // 虚拟化后 bounds_for_item 恒 miss（uniform_list 不填 child_bounds），
        // 拖框命中统一走 virtual_grid_geometry 同源数学推算：
        // 容器 bounds（真实原点+宽度）从 ScrollHandle 取，行/列位置按同一几何公式算。
        let scroll_handle = if side == "left" {
            self.left_scroll_handle.0.borrow().base_handle.clone()
        } else {
            self.right_scroll_handle.0.borrow().base_handle.clone()
        };
        let scroll_offset = scroll_handle.offset();
        let mut range = HashSet::new();
        let mut focused = None;
        // 容器内容区原点（窗口坐标）= bounds.origin - 滚动偏移
        let view_bounds = scroll_handle.bounds();
        let origin_x = pixel_value(view_bounds.origin.x) - pixel_value(scroll_offset.x);
        let origin_y = pixel_value(view_bounds.origin.y) - pixel_value(scroll_offset.y);
        let pane_w = f32::from(view_bounds.size.width).max(1.);
        let view_mode = self.view_mode_for(side);
        let geo = virtual_grid_geometry(
            view_mode,
            self.file_font_size,
            self.row_spacing,
            pane_w,
            visible_paths.len(),
        );
        for (index, path) in visible_paths.iter().enumerate() {
            let (l, t, r, b) = geo.cell_rect(index);
            // List/Details 退化分支 cell_w = pane_w、pad=0，天然正确
            let item = (origin_x + l, origin_y + t, origin_x + r, origin_y + b);
            if rect_intersects(selection, item) {
                range.insert(path.clone());
                focused = Some(path.clone());
            }
        }
        if side == "left" {
            if modifiers.control {
                self.left_selected.extend(range);
            } else {
                self.left_selected = range;
            }
            self.left_focused = focused;
        } else {
            if modifiers.control {
                self.right_selected.extend(range);
            } else {
                self.right_selected = range;
            }
            self.right_focused = focused;
        }
    }

    fn finish_drag_select(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = self.drag_select.take() {
            if state.active {
                self.apply_drag_selection(
                    state.side,
                    state.start,
                    event.position,
                    &state.visible_paths,
                    state.modifiers,
                    window,
                );
            }
            self.suppress_blank_click = state.active;
            cx.notify();
        }
    }

    fn on_blank_area_mouse_down(
        &mut self,
        side: &'static str,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        // 滚动容器与 pane-hit 父容器都注册了本处理器，gpui 冒泡阶段两者都会命中
        // （is_hovered 对嵌套 hitbox 均为 true），必须 stop_propagation 保证只触发一次
        match event.button {
            MouseButton::Right => {
                cx.stop_propagation();
                self.open_merged_blank_context_menu_at(side, cx);
            }
            MouseButton::Navigate(NavigationDirection::Back) => {
                cx.stop_propagation();
                self.go_back(side, cx);
            }
            MouseButton::Navigate(NavigationDirection::Forward) => {
                cx.stop_propagation();
                self.go_forward(side, cx);
            }
            _ => {}
        }
    }

    fn on_refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh(cx);
    }
    fn on_up(&mut self, _: &GoUp, _: &mut Window, cx: &mut Context<Self>) {
        self.go_up_side(self.active_side, cx);
    }
    fn on_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.go_back(self.active_side, cx);
    }
    fn on_forward(&mut self, _: &GoForward, _: &mut Window, cx: &mut Context<Self>) {
        self.go_forward(self.active_side, cx);
    }
    fn on_split(&mut self, _: &ToggleSplit, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_split(cx);
    }
    fn on_next_view(&mut self, _: &NextView, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_view(1, cx);
    }
    fn on_previous_view(&mut self, _: &PreviousView, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_view(-1, cx);
    }
    fn on_new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        // 对活动栏生效：右栏激活时开右栏标签
        if self.split && self.active_side == "right" {
            self.add_right_tab(cx);
        } else {
            self.add_tab(cx);
        }
    }
    fn on_close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_side == "right" && self.split {
            self.close_right_tab(self.right_active_tab, cx);
        } else {
            self.close_tab(self.active_tab, cx);
        }
    }
    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.show_sidebar = !self.show_sidebar;
        self.status = if self.show_sidebar {
            "侧边栏已显示"
        } else {
            "侧边栏已隐藏"
        }
        .to_string();
        self.save_config();
        cx.notify();
    }
    fn on_new_folder(&mut self, _: &NewFolder, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_folder_open {
            self.new_folder_input.update(cx, |input, cx| input.insert_newline(window, cx));
            window.focus(&self.new_folder_input.read(cx).focus_handle(cx));
            return;
        }
        self.open_new_item(NewItemKind::Folder, window, cx);
    }
    fn on_new_text_file(&mut self, _: &NewTextFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_folder_open {
            self.new_folder_input.update(cx, |input, cx| input.insert_newline(window, cx));
            window.focus(&self.new_folder_input.read(cx).focus_handle(cx));
            return;
        }
        self.open_new_item(NewItemKind::TextFile, window, cx);
    }
    fn open_new_item(&mut self, kind: NewItemKind, window: &mut Window, cx: &mut Context<Self>) {
        self.new_folder_open = true;
        self.new_item_kind = kind;
        self.new_folder_input
            .update(cx, |input, _| input.set_value(if kind == NewItemKind::Folder { "新建文件夹" } else { "新建文本文档" }));
        window.focus(&self.new_folder_input.read(cx).focus_handle(cx));
        self.status = format!("每行新建一个{}；Shift+Enter / Ctrl+N 换行，Enter 创建", kind.label());
        cx.notify();
    }
    fn on_hidden(&mut self, _: &ToggleHidden, _: &mut Window, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.status = if self.show_hidden {
            "已显示隐藏文件"
        } else {
            "已隐藏隐藏文件"
        }
        .to_string();
        self.save_config();
        cx.notify();
    }
    fn on_folder_order(&mut self, _: &CycleFolders, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_folders(cx);
    }
    fn on_copy_other(&mut self, _: &CopyToOther, _: &mut Window, cx: &mut Context<Self>) {
        self.transfer_to_other(false, cx);
    }
    fn on_move_other(&mut self, _: &MoveToOther, _: &mut Window, cx: &mut Context<Self>) {
        self.transfer_to_other(true, cx);
    }
    fn on_copy_paths(&mut self, _: &CopyPaths, _: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            self.status = "请先选择要复制路径的文件或文件夹".to_string();
            cx.notify();
            return;
        }
        let text = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.status = format!("已复制 {} 项路径", paths.len());
        cx.notify();
    }
    fn on_select_all_entries(
        &mut self,
        _: &SelectAllEntries,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = self.filter_input_for(self.active_side).read(cx).value();
        let entries = if self.active_side == "right" && self.split {
            self.arranged_entries(&self.right_entries, &query, "right")
        } else {
            self.arranged_entries(&self.left_entries, &query, "left")
        };
        let paths = entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<HashSet<_>>();
        let focused = entries.last().map(|entry| entry.path.clone());
        let anchor = entries.first().map(|entry| entry.path.clone());
        if self.active_side == "right" && self.split {
            self.right_selected = paths;
            self.right_focused = focused;
            self.right_anchor = anchor;
        } else {
            self.left_selected = paths;
            self.left_focused = focused;
            self.left_anchor = anchor;
        }
        self.status = format!("已全选 {} 项", entries.len());
        cx.notify();
    }
    fn on_open_properties(&mut self, _: &OpenProperties, _: &mut Window, cx: &mut Context<Self>) {
        self.open_windows_properties(cx);
    }
    fn on_copy_selected(&mut self, _: &CopySelected, _: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            self.status = "请先选择要复制的文件或文件夹".to_string();
        } else {
            self.status = match put_files_on_windows_clipboard(&paths, DROPEFFECT_COPY) {
                Ok(()) => format!("已复制 {} 项", paths.len()),
                Err(error) => format!("复制失败：{error}"),
            };
        }
        cx.notify();
    }
    fn on_cut_selected(&mut self, _: &CutSelected, _: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            self.status = "请先选择要剪切的文件或文件夹".to_string();
        } else {
            self.status = match put_files_on_windows_clipboard(&paths, DROPEFFECT_MOVE) {
                Ok(()) => format!("已剪切 {} 项", paths.len()),
                Err(error) => format!("剪切失败：{error}"),
            };
        }
        cx.notify();
    }
    fn on_paste_files(&mut self, _: &PasteFiles, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.active_side == "right" && self.split {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        match files_from_windows_clipboard() {
            Ok((paths, move_items)) if !paths.is_empty() => {
                self.drop_files_to(&paths, &target, !move_items, cx)
            }
            Ok(_) => {
                self.status = "剪贴板中没有文件或文件夹".to_string();
                cx.notify();
            }
            Err(error) => {
                self.status = format!("读取剪贴板失败：{error}");
                cx.notify();
            }
        }
    }
    fn create_text_file(&mut self, cx: &mut Context<Self>) {
        let folder = if self.context_menu_side == "right" && self.split {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        if is_virtual_path(&folder) || !folder.is_dir() {
            self.status = "当前位置不能新建 TXT 文档".to_string();
            cx.notify();
            return;
        }
        let mut path = folder.join("新建文本文档.txt");
        let mut index = 2usize;
        while path.exists() {
            path = folder.join(format!("新建文本文档 ({index}).txt"));
            index += 1;
        }
        self.status = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => {
                self.refresh(cx);
                self.set_single_selection(self.context_menu_side, path.clone());
                format!(
                    "已新建 {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
            }
            Err(error) => format!("新建 TXT 文档失败：{error}"),
        };
        cx.notify();
    }
    fn on_focus_filter(&mut self, _: &FocusFilter, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.filter_input_for(self.active_side).clone();
        window.focus(&input.read(cx).focus_handle(cx));
    }
    fn on_focus_address(&mut self, _: &FocusAddress, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_address_for_side(self.active_side, window, cx);
    }
    fn focus_address_for_side(
        &mut self,
        side: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        self.address_side = side;
        self.address_editing = true;
        let path = if side == "right" && self.split {
            self.right_path.display().to_string()
        } else {
            self.current_path().display().to_string()
        };
        // set_value 会触发 observe→建议探测，标记跳过（当前路径已知，无需探测）
        self.address_suggestions_suppress = true;
        self.address_suggestions.clear();
        self.address_input
            .update(cx, |input, _| input.set_value(path));
        window.focus(&self.address_input.read(cx).focus_handle(cx));
        cx.notify();
    }
    fn on_clear_transient(
        &mut self,
        _: &ClearTransient,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.context_menu_open {
            self.context_menu_open = false;
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.new_folder_open {
            self.new_folder_open = false;
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.rename_open {
            self.rename_open = false;
            self.rename_target = None;
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.batch_rename_open {
            self.batch_rename_open = false;
            self.batch_rename_targets.clear();
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.show_settings || self.show_shortcuts {
            self.show_settings = false;
            self.show_shortcuts = false;
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        if self.address_editing {
            self.address_editing = false;
            window.focus(&self.focus_handle);
            cx.notify();
            return;
        }
        // Esc 优先退出搜索（Ctrl+F 筛选）：清空筛选并把焦点交还文件区
        let filter_input = self.filter_input_for(self.active_side).clone();
        let filter_focused = filter_input.read(cx).focus_handle(cx).is_focused(window);
        let filter_active = !filter_input.read(cx).value().trim().is_empty();
        if filter_focused || filter_active {
            filter_input.update(cx, |input, _| input.clear());
            window.focus(&self.focus_handle);
            self.status = "已退出搜索".to_string();
            cx.notify();
            return;
        }
        if !self.left_selected.is_empty() || !self.right_selected.is_empty() {
            self.left_selected.clear();
            self.right_selected.clear();
            self.left_focused = None;
            self.right_focused = None;
            self.status = "已取消选择".to_string();
            window.focus(&self.focus_handle);
            cx.notify();
        }
    }
    fn on_submit_address(
        &mut self,
        _: &SubmitAddress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.batch_rename_open {
            let base = self.batch_rename_input.read(cx).value().trim().to_string();
            let side = self.active_side;
            let start = self.batch_rename_start;
            let targets = std::mem::take(&mut self.batch_rename_targets);
            self.batch_rename_open = false;
            window.focus(&self.focus_handle);
            let undo_targets = targets.clone();
            self.status = format!("正在批量重命名 {} 项…", targets.len());
            cx.notify();
            let task = cx.background_executor().spawn(async move {
                batch_rename_files(&targets, &base, start)
            });
            cx.spawn(async move |this, cx| {
                let (renamed, errors) = task.await;
                wait_out_of_ole_modal(cx).await;
                let _ = this.update(cx, |this, cx| {
                    let ok_count = renamed.len();
                    if !renamed.is_empty() {
                        let mut undo_old = undo_targets;
                        undo_old.truncate(renamed.len());
                        let last_new = renamed.last().cloned();
                        this.push_undo(UndoEntry::RenameBatch {
                            from: renamed,
                            to: undo_old,
                        });
                        if let Some(last) = last_new {
                            this.set_single_selection(side, last);
                        }
                    }
                    this.status = if errors.is_empty() {
                        format!("已批量重命名 {ok_count} 项")
                    } else {
                        format!(
                            "已重命名 {ok_count} 项，{} 项失败：{}",
                            errors.len(),
                            errors.join("；")
                        )
                    };
                    this.reload_visible_async(cx);
                });
            })
            .detach();
            return;
        }
        if self.rename_open {
            let new_name = self.rename_input.read(cx).value().trim().to_string();
            let rename_side = self.active_side;
            let operation = self
                .rename_target
                .as_ref()
                .and_then(|target| {
                    target
                        .parent()
                        .map(|parent| (target.clone(), parent.join(&new_name)))
                })
                .ok_or_else(|| "没有选中项目".to_string());
            let undo_rename_from = self.rename_target.clone();
            self.rename_open = false;
            self.rename_target = None;
            window.focus(&self.focus_handle);
            self.status = "正在重命名…".to_string();
            cx.notify();
            let task = cx.background_executor().spawn(async move {
                operation.and_then(|(target, new_path)| {
                    if new_name.is_empty() {
                        Err("名称不能为空".to_string())
                    } else if target == new_path {
                        Ok(new_path)
                    } else if new_path.exists() {
                        Err("同名文件或文件夹已经存在".to_string())
                    } else {
                        fs::rename(&target, &new_path)
                            .map(|_| new_path)
                            .map_err(|error| error.to_string())
                    }
                })
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                wait_out_of_ole_modal(cx).await;
                let _ = this.update(cx, |this, cx| {
                    this.status = match result {
                        Ok(new_path) => {
                            this.set_single_selection(rename_side, new_path.clone());
                            if let Some(old_path) = undo_rename_from {
                                this.push_undo(UndoEntry::Rename {
                                    from: new_path,
                                    to: old_path,
                                });
                            }
                            "已重命名".to_string()
                        }
                        Err(error) => format!("重命名失败：{error}"),
                    };
                    this.reload_visible_async(cx);
                });
            })
            .detach();
            return;
        }
        if self.new_folder_open {
            let input_value = self.new_folder_input.read(cx).value();
            let kind = self.new_item_kind;
            let names = match new_item_names(&input_value, kind) {
                Ok(names) => names,
                Err(error) => { self.status = error; cx.notify(); return; }
            };
            let side = self.active_side;
            let base = if self.active_side == "right" {
                self.right_path.clone()
            } else {
                self.current_path().to_path_buf()
            };
            if is_virtual_path(&base) {
                self.new_folder_open = false;
                self.status = format!("虚拟位置不能新建{}", kind.label());
                window.focus(&self.focus_handle);
                cx.notify();
                return;
            }
            self.new_folder_open = false;
            window.focus(&self.focus_handle);
            self.status = format!("正在新建{}…", kind.label());
            cx.notify();
            let task = cx.background_executor().spawn(async move { create_new_items(&base, &names, kind) });
            cx.spawn(async move |this, cx| {
                let (created, errors) = task.await;
                wait_out_of_ole_modal(cx).await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(last) = created.last() {
                        this.set_single_selection(side, last.clone());
                    }
                    if !created.is_empty() {
                        this.push_undo(UndoEntry::RemoveNew(created.clone()));
                    }
                    this.status = if errors.is_empty() { format!("已创建 {} 个{}", created.len(), kind.label()) }
                        else { format!("已创建 {} 个{}，{} 项失败：{}", created.len(), kind.label(), errors.len(), errors[0]) };
                    this.reload_visible_async(cx);
                });
            })
            .detach();
            return;
        }
        if !self.address_editing {
            // Enter：打开/进入选中项（与双击一致），设置/快捷键面板里不触发
            if self.show_settings || self.show_shortcuts {
                return;
            }
            self.open_selected(cx);
            return;
        }
        let path = path_from_user_input(&self.address_input.read(cx).value());
        if can_open_directory(&path) {
            if self.address_side == "right" && self.split {
                self.navigate_right(path, cx);
            } else {
                self.navigate_left(path, cx);
            }
            self.address_editing = false;
            window.focus(&self.focus_handle);
        } else {
            self.status = "路径不存在或不可访问；网络路径可输入 \\\\主机\\共享名".to_string();
            cx.notify();
        }
    }
    fn on_open_settings(&mut self, _: &OpenSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.show_settings = !self.show_settings;
        self.show_shortcuts = false;
        cx.notify();
    }
    fn on_open_shortcuts(&mut self, _: &OpenShortcuts, _: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        self.show_settings = false;
        cx.notify();
    }
    fn on_toggle_preview(&mut self, _: &TogglePreview, _: &mut Window, cx: &mut Context<Self>) {
        let open = if self.active_side == "right" {
            self.right_preview_open = !self.right_preview_open;
            self.right_preview_open
        } else {
            self.left_preview_open = !self.left_preview_open;
            self.left_preview_open
        };
        self.status = format!(
            "{}栏预览{}",
            if self.active_side == "right" {
                "右"
            } else {
                "左"
            },
            if open { "已打开" } else { "已关闭" }
        );
        cx.notify();
    }
    fn on_toggle_favorite(&mut self, _: &ToggleFavorite, _: &mut Window, cx: &mut Context<Self>) {
        let path = self
            .selected_path()
            .filter(|path| can_open_directory(path))
            .unwrap_or_else(|| {
                if self.active_side == "right" {
                    self.right_path.clone()
                } else {
                    self.current_path().to_path_buf()
                }
            });
        if !can_open_directory(&path) {
            self.status = "当前不是可收藏的文件夹".to_string();
            cx.notify();
            return;
        }
        if let Some(index) = self.favorites.iter().position(|item| item == &path) {
            self.favorites.remove(index);
            self.status = "已取消收藏".to_string();
        } else {
            self.favorites.push(path);
            self.status = "已收藏文件夹".to_string();
        }
        self.save_config();
        cx.notify();
    }

    fn open_context_menu_at(
        &mut self,
        side: &'static str,
        path: PathBuf,
        position: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        self.prepare_context_target(side, &path);
        self.context_menu_position = position;
        self.context_menu_open = true;
        cx.notify();
    }

    fn prepare_context_target(&mut self, side: &'static str, path: &Path) {
        self.active_side = side;
        self.context_menu_side = side;
        self.context_menu_blank = false;
        if !self.is_selected(side, path) {
            self.set_single_selection(side, path.to_path_buf());
        } else if side == "left" {
            self.left_focused = Some(path.to_path_buf());
        } else {
            self.right_focused = Some(path.to_path_buf());
        }
    }

    fn begin_context_target(
        &mut self,
        side: &'static str,
        path: PathBuf,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if is_virtual_path(&path) {
            // 虚拟路径保留 GPUI 菜单（含 100ms 拖拽打断窗口）
            self.prepare_context_target(side, &path);
            self.pending_context_target = Some((side, path.clone()));
            self.context_menu_position = Some(position);
            cx.notify();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                wait_out_of_ole_modal(cx).await;
                let _ = this.update(cx, |this, cx| {
                    let still_pending = this.pending_context_target.as_ref().is_some_and(
                        |(pending_side, pending_path)| {
                            *pending_side == side && pending_path == &path
                        },
                    );
                    if still_pending {
                        this.finish_context_target(position, cx);
                    }
                });
            })
            .detach();
        } else {
            // 真实路径直接弹合并菜单，不再有 100ms 固定延迟
            self.open_merged_context_menu_at(side, path, cx);
        }
    }

    fn finish_context_target(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some((side, path)) = self.pending_context_target.take() {
            self.open_context_menu_at(side, path, Some(position), cx);
        }
    }

    fn open_blank_context_menu_at(
        &mut self,
        side: &'static str,
        position: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        self.active_side = side;
        self.context_menu_side = side;
        self.context_menu_blank = true;
        self.suppress_blank_click = true;
        self.context_menu_position = position;
        self.context_menu_open = true;
        cx.notify();
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path() else {
            return;
        };
        if is_network_path(&path) {
            let _ = std::process::Command::new("explorer")
                .arg("shell:NetworkPlacesFolder")
                .spawn();
            self.status = "已打开网络".to_string();
            cx.notify();
            return;
        }
        if is_recycle_bin_path(&path) {
            let _ = std::process::Command::new("explorer")
                .arg("shell:RecycleBinFolder")
                .spawn();
            self.status = "已打开回收站".to_string();
            cx.notify();
            return;
        }
        if path.is_dir() {
            if self.active_side == "left" {
                self.navigate_left(path, cx);
            } else {
                self.navigate_right(path, cx);
            }
        } else {
            self.remember_recent(&path);
            self.save_config();
            let opened = shell_open_default(&path);
            self.status = if opened {
                "已用系统默认程序打开"
            } else {
                "系统默认程序打开失败"
            }
            .to_string();
            cx.notify();
        }
    }

    fn open_selected_in_other(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path() else {
            self.status = "请先选择文件夹".to_string();
            cx.notify();
            return;
        };
        if !path.is_dir() && !is_virtual_path(&path) {
            self.status = "只能在另一栏打开文件夹".to_string();
            cx.notify();
            return;
        }
        if !self.split {
            self.split = true;
        }
        if is_virtual_path(&path) {
            if self.active_side == "left" {
                self.open_virtual("right", path, cx);
            } else {
                self.open_virtual("left", path, cx);
            }
        } else if self.active_side == "left" {
            self.navigate_right(path, cx);
        } else {
            self.navigate_left(path, cx);
        }
    }

    fn open_in_explorer(&mut self, cx: &mut Context<Self>) {
        let argument = if let Some(path) = self.selected_path() {
            if path.is_dir() {
                path
            } else {
                path.parent().map(Path::to_path_buf).unwrap_or(path)
            }
        } else if self.active_side == "left" {
            self.current_path().to_path_buf()
        } else {
            self.right_path.clone()
        };
        if is_network_path(&argument) {
            let _ = std::process::Command::new("explorer")
                .arg("shell:NetworkPlacesFolder")
                .spawn();
            return;
        }
        if is_recycle_bin_path(&argument) {
            let _ = std::process::Command::new("explorer")
                .arg("shell:RecycleBinFolder")
                .spawn();
            return;
        }
        let _ = std::process::Command::new("explorer").arg(argument).spawn();
        self.status = "已在资源管理器打开".to_string();
        cx.notify();
    }

    fn reveal_in_explorer(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path() else {
            self.open_in_explorer(cx);
            return;
        };
        let _ = if path.exists() {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
        } else {
            std::process::Command::new("explorer")
                .arg(path.parent().map(Path::to_path_buf).unwrap_or(path))
                .spawn()
        };
        self.status = "已在资源管理器中定位".to_string();
        cx.notify();
    }

    fn open_windows_properties(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path() else {
            self.status = "请先选择要查看属性的项目".to_string();
            cx.notify();
            return;
        };
        self.status = if shell_execute_verb(&path, "properties") {
            "已打开 Windows 属性"        } else {
            "无法打开 Windows 属性"
        }
        .to_string();
        cx.notify();
    }

    fn open_native_context_menu(&mut self, cx: &mut Context<Self>) {
        let target = self.selected_path().unwrap_or_else(|| {
            if self.context_menu_side == "right" {
                self.right_path.clone()
            } else {
                self.current_path().to_path_buf()
            }
        });
        self.open_native_context_menu_for(target, cx);
    }

    fn spawn_shell_menu_helper(&mut self, request: ShellMenuRequest, cx: &mut Context<Self>) {
        for path in &request.paths {
            if is_virtual_path(path)
                || (!path.exists() && !can_open_directory(path))
            {
                self.status = "当前项目不能打开 Windows 原生右键菜单".to_string();
                cx.notify();
                return;
            }
        }
        let helper = std::env::current_exe().and_then(|executable| {
            let mut command = std::process::Command::new(executable);
            command.arg("--shell-context-menu");
            if request.blank {
                command.arg("--blank");
            }
            command.arg(if request.side == "right" {
                "--right"
            } else {
                "--left"
            });
            if request.include_third_party {
                command.arg("--third-party");
            }
            command.arg("--paths");
            for path in &request.paths {
                command.arg(path);
            }
            let child = command.spawn()?;
            unsafe {
                let _ = AllowSetForegroundWindow(child.id());
            }
            Ok(child)
        });
        match helper {
            Ok(_) => self.status = "已打开 Windows Shell 右键菜单".to_string(),
            Err(error) => self.status = format!("Windows 右键菜单失败：{error}"),
        }
        cx.notify();
    }

    /// 真实路径的文件项右键：合并菜单（自定义项 + Shell 原生项一次弹出）。
    /// 多选时把全部选中路径交给 helper，Shell 菜单与自定义命令都能作用于多选。
    fn open_merged_context_menu_at(
        &mut self,
        side: &'static str,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if is_virtual_path(&path) {
            // 虚拟路径（此电脑/网络/回收站）回退 GPUI 自绘菜单
            self.open_context_menu_at(side, path, None, cx);
            return;
        }
        self.prepare_context_target(side, &path);
        // 关闭可能残留的 GPUI 菜单（如刚在虚拟路径上弹过），避免与 Shell 菜单叠加
        self.context_menu_open = false;
        let selected: Vec<PathBuf> = if side == "left" {
            self.left_selected.iter().cloned().collect()
        } else {
            self.right_selected.iter().cloned().collect()
        };
        let paths = if selected.contains(&path) && !selected.is_empty() {
            selected
        } else {
            vec![path]
        };
        let request = ShellMenuRequest {
            blank: false,
            side,
            paths,
            include_third_party: self.shell_menu_third_party,
        };
        self.spawn_shell_menu_helper(request, cx);
    }

    /// 空白处右键：Shell 文件夹背景菜单（自带粘贴/新建/属性）+ 自定义项
    fn open_merged_blank_context_menu_at(
        &mut self,
        side: &'static str,
        cx: &mut Context<Self>,
    ) {
        let folder = if side == "right" && self.split {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        if is_virtual_path(&folder) || !folder.is_dir() {
            self.open_blank_context_menu_at(side, None, cx);
            return;
        }
        self.active_side = side;
        self.context_menu_side = side;
        // 关闭可能残留的 GPUI 菜单，避免与 Shell 菜单叠加
        self.context_menu_open = false;
        let request = ShellMenuRequest {
            blank: true,
            side,
            paths: vec![folder],
            include_third_party: self.shell_menu_third_party,
        };
        self.spawn_shell_menu_helper(request, cx);
    }

    fn open_native_context_menu_for(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        let target = normalize_path(target);
        let request = ShellMenuRequest {
            blank: false,
            side: self.context_menu_side,
            paths: vec![target],
            include_third_party: self.shell_menu_third_party,
        };
        self.spawn_shell_menu_helper(request, cx);
    }

    fn open_command_prompt_here(&mut self, cx: &mut Context<Self>) {
        let folder = if self.context_menu_side == "right" && self.split {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        if is_virtual_path(&folder) || !can_open_directory(&folder) {
            self.status = "当前位置不能打开命令提示符".to_string();
            cx.notify();
            return;
        }
        let _ = std::process::Command::new("cmd.exe")
            .arg("/K")
            .arg(format!("cd /d \"{}\"", folder.display()))
            .current_dir(&folder)
            .spawn();
        self.status = "已在此处打开命令提示符".to_string();
        cx.notify();
    }

    /// Shell 合并菜单里自定义项的执行入口（经 TCP 通道从 helper 回传）。
    /// 命令作用于发出右键的那一栏的当前选择。
    fn execute_menu_command(
        &mut self,
        command: &str,
        side: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 命令必须作用于原栏，避免双栏时落在错误一侧
        self.active_side = side;
        self.context_menu_side = side;
        // 查看方式子菜单
        if let Some(argument) = command.strip_prefix("view-") {
            let view_mode = match argument {
                "details" => ViewMode::Details,
                "list" => ViewMode::List,
                "columns" => ViewMode::Columns,
                "m-icons" => ViewMode::MIcons,
                "l-icons" => ViewMode::LIcons,
                _ => ViewMode::XLIcons,
            };
            self.set_view_mode_for(side, view_mode);
            self.view_epoch = self.view_epoch.wrapping_add(1);
            self.status = format!("查看方式：{}", view_mode.label());
            cx.notify();
            return;
        }
        // 排序方式子菜单
        if let Some(argument) = command.strip_prefix("sort-") {
            let column = match argument {
                "name" => SortColumn::Name,
                "type" => SortColumn::Type,
                "modified" => SortColumn::Modified,
                _ => SortColumn::Size,
            };
            self.sort_by(side, column, cx);
            return;
        }
        match command {
            "open" => self.open_selected(cx),
            "open-other" => self.open_selected_in_other(cx),
            "copy" => self.on_copy_selected(&CopySelected, window, cx),
            "cut" => self.on_cut_selected(&CutSelected, window, cx),
            "paste" => self.on_paste_files(&PasteFiles, window, cx),
            "rename" => self.on_rename_selected(&RenameSelected, window, cx),
            "delete" => self.on_delete_selected(&DeleteSelected, window, cx),
            "permanent-delete" => {
                self.on_permanent_delete_selected(&PermanentDeleteSelected, window, cx)
            }
            "new-folder" => self.on_new_folder(&NewFolder, window, cx),
            "new-text" => self.create_text_file(cx),
            "copy-path" => self.on_copy_paths(&CopyPaths, window, cx),
            "properties" => self.open_windows_properties(cx),
            "favorite" => self.on_toggle_favorite(&ToggleFavorite, window, cx),
            "explorer" => self.open_in_explorer(cx),
            "reveal" => self.reveal_in_explorer(cx),
            "terminal" => self.open_command_prompt_here(cx),
            "refresh" => self.refresh(cx),
            _ => {
                self.status = format!("未知菜单命令：{command}");
            }
        }
        cx.notify();
    }

    fn on_rename_selected(
        &mut self,
        _: &RenameSelected,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let targets = self.selected_paths();
        // 多选：批量重命名（名称_001、名称_002…）
        if targets.len() > 1 {
            let sample_stem = targets
                .first()
                .and_then(|path| path.file_stem())
                .and_then(|stem| stem.to_str())
                .unwrap_or("名称")
                .to_string();
            self.batch_rename_targets = targets;
            self.batch_rename_start = 1;
            let stem_len = sample_stem.len();
            self.batch_rename_input.update(cx, |input, _| {
                input.set_value_select_until(sample_stem, stem_len)
            });
            self.batch_rename_open = true;
            focus_fileflow_window();
            window.focus(&self.batch_rename_input.read(cx).focus_handle(cx));
            cx.notify();
            return;
        }
        let target = if self.active_side == "right" {
            self.right_focused
                .clone()
                .filter(|path| self.right_selected.contains(path))
        } else {
            self.left_focused
                .clone()
                .filter(|path| self.left_selected.contains(path))
        }
        .or_else(|| self.selected_path());
        let Some(target) = target else {
            self.status = "请先选择要重命名的项目".to_string();
            cx.notify();
            return;
        };
        let current_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        // Windows 资源管理器习惯：只选中主文件名，扩展名保留
        // （xxx.jpg 选中 "xxx"，直接打字只改名字部分）
        let stem_len = current_name
            .rfind('.')
            .filter(|&dot| dot > 0)
            .unwrap_or(current_name.len());
        self.rename_input
            .update(cx, |input, _| input.set_value_select_until(current_name, stem_len));
        self.rename_target = Some(target);
        self.rename_open = true;
        // 右键菜单由 helper 子进程弹出：菜单关闭后 Windows 把前台判给了
        // helper，主窗口（连同重命名弹框）会被压到其他窗口后面。这里
        // 主动把前台拿回来，弹框才能出现在最前。
        focus_fileflow_window();
        window.focus(&self.rename_input.read(cx).focus_handle(cx));
        cx.notify();
    }
    fn on_delete_selected(&mut self, _: &DeleteSelected, _: &mut Window, cx: &mut Context<Self>) {
        let targets = self.selected_paths();
        if targets.is_empty() {
            self.status = "请先选择要删除的项目".to_string();
            cx.notify();
            return;
        }
        self.run_shell_operation(ShellOperationKind::Delete, targets, None, cx);
    }

    /// Shift+Del：永久删除选中项（不进回收站、Ctrl+Z 无法撤销）。
    fn on_permanent_delete_selected(
        &mut self,
        _: &PermanentDeleteSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let targets = self.selected_paths();
        if targets.is_empty() {
            self.status = "请先选择要删除的项目".to_string();
            cx.notify();
            return;
        }
        // 不可逆操作，先把数量和"不可撤销"说清楚
        self.status = format!("永久删除 {} 项（不可撤销）…", targets.len());
        cx.notify();
        // 不 push_undo：永久删除不进撤销栈
        self.run_shell_operation(ShellOperationKind::PermanentDelete, targets, None, cx);
    }
    fn push_undo(&mut self, entry: UndoEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > 32 {
            self.undo_stack.remove(0);
        }
    }
    /// Ctrl+Z：撤销最近一次 删除(回收站恢复)/重命名/新建文件夹。
    /// 用 IFileOperation 的 verb 走 Shell 撤销通道。
    fn on_navigate_up(&mut self, _: &NavigateUp, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate_selection(-1, 0, window, cx);
    }
    fn on_navigate_down(&mut self, _: &NavigateDown, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate_selection(1, 0, window, cx);
    }
    fn on_navigate_left(&mut self, _: &NavigateLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate_selection(0, -1, window, cx);
    }
    fn on_navigate_right(&mut self, _: &NavigateRight, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate_selection(0, 1, window, cx);
    }

    fn on_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.undo_stack.pop() else {
            self.status = "没有可撤销的操作".to_string();
            cx.notify();
            return;
        };
        match entry {
            UndoEntry::Restore(paths) => {
                self.status = format!("正在恢复 {} 项…", paths.len());
                cx.notify();
                let task = cx.background_executor().spawn(async move {
                    undo_recycle_restore(&paths)
                });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    wait_out_of_ole_modal(cx).await;
                    let _ = this.update(cx, |this, context| {
                        this.status = match result {
                            Ok(()) => "已从回收站恢复".to_string(),
                            Err(error) => format!("恢复失败：{error}"),
                        };
                        this.reload_visible_async(context);
                    });
                })
                .detach();
            }
            UndoEntry::Rename { from, to } => {
                self.status = "正在撤销重命名…".to_string();
                cx.notify();
                let task = cx.background_executor().spawn(async move {
                    if !from.exists() {
                        return Err(format!("原文件已不存在：{}", from.display()));
                    }
                    fs::rename(&from, &to)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    wait_out_of_ole_modal(cx).await;
                    let _ = this.update(cx, |this, context| {
                        this.status = match result {
                            Ok(()) => "已撤销重命名".to_string(),
                            Err(error) => format!("撤销重命名失败：{error}"),
                        };
                        this.reload_visible_async(context);
                    });
                })
                .detach();
            }
            UndoEntry::RenameBatch { from, to } => {
                self.status = format!("正在撤销批量重命名 {} 项…", from.len());
                cx.notify();
                // 两阶段：先全部改名到临时名避免互相占位，再改回原名
                let task = cx.background_executor().spawn(async move {
                    let mut failures = 0usize;
                    for (index, path) in from.iter().enumerate() {
                        let temp = path.with_file_name(format!(
                            "__fileflow_batch_undo_{index}__{}",
                            path.extension()
                                .and_then(|e| e.to_str())
                                .map(|e| format!(".{e}"))
                                .unwrap_or_default()
                        ));
                        if fs::rename(path, &temp).is_ok() {
                            if let Some(original) = to.get(index) {
                                if fs::rename(&temp, original).is_err() {
                                    failures += 1;
                                }
                            } else {
                                let _ = fs::rename(&temp, path);
                            }
                        } else {
                            failures += 1;
                        }
                    }
                    failures
                });
                cx.spawn(async move |this, cx| {
                    let failures = task.await;
                    wait_out_of_ole_modal(cx).await;
                    let _ = this.update(cx, |this, context| {
                        this.status = if failures == 0 {
                            "已撤销批量重命名".to_string()
                        } else {
                            format!("撤销批量重命名：{failures} 项失败")
                        };
                        this.reload_visible_async(context);
                    });
                })
                .detach();
            }
            UndoEntry::RemoveNew(paths) => {
                self.status = "正在撤销新建…".to_string();
                cx.notify();
                let task = cx.background_executor().spawn(async move {
                    let mut removed = Vec::new();
                    for path in &paths {
                        if path.is_dir()
                            && fs::remove_dir(path).is_ok()
                        {
                            removed.push(path.clone());
                        }
                    }
                    removed
                });
                cx.spawn(async move |this, cx| {
                    let removed = task.await;
                    wait_out_of_ole_modal(cx).await;
                    let _ = this.update(cx, |this, context| {
                        this.status = format!("已撤销新建 {} 项", removed.len());
                        this.reload_visible_async(context);
                    });
                })
                .detach();
            }
        }
    }
    fn on_view_details(&mut self, _: &ViewDetails, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::Details, cx);
    }
    fn on_view_list(&mut self, _: &ViewList, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::List, cx);
    }
    fn on_view_columns(&mut self, _: &ViewColumns, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::Columns, cx);
    }
    fn on_view_m_icons(&mut self, _: &ViewMIcons, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::MIcons, cx);
    }
    fn on_view_l_icons(&mut self, _: &ViewLIcons, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::LIcons, cx);
    }
    fn on_view_xl_icons(&mut self, _: &ViewXLIcons, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view(ViewMode::XLIcons, cx);
    }

    fn transfer_to_other(&mut self, move_item: bool, cx: &mut Context<Self>) {
        if !self.split {
            self.status = "请先开启双栏".to_string();
            cx.notify();
            return;
        }
        let sources = self.selected_paths();
        if sources.is_empty() {
            self.status = "请先选择文件或文件夹".to_string();
            cx.notify();
            return;
        }
        let from_left = self.active_side == "left";
        let target_dir = if from_left {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        self.run_shell_operation(
            if move_item {
                ShellOperationKind::Move
            } else {
                ShellOperationKind::Copy
            },
            sources,
            Some(target_dir),
            cx,
        );
    }

    fn drop_files_to(
        &mut self,
        sources: &[PathBuf],
        target_dir: &Path,
        copy: bool,
        cx: &mut Context<Self>,
    ) {
        if is_virtual_path(target_dir) || !target_dir.is_dir() {
            self.status = "只能拖放到文件夹".to_string();
            cx.notify();
            return;
        }
        let valid_sources = sources
            .iter()
            .filter(|source| {
                *source != target_dir
                    && (copy || source.parent() != Some(target_dir))
                    && !target_dir.starts_with(*source)
            })
            .cloned()
            .collect::<Vec<_>>();
        if valid_sources.is_empty() {
            // 同目录复制粘贴不在此列：copy=true 时同目录源是合法的（自动改名），
            // 走到这里说明确实没有可操作的源（例如剪切到自身目录）
            self.status = "目标位置与源位置相同，无需移动".to_string();
            cx.notify();
            return;
        }
        self.run_shell_operation(
            if copy {
                ShellOperationKind::Copy
            } else {
                ShellOperationKind::Move
            },
            valid_sources,
            Some(target_dir.to_path_buf()),
            cx,
        );
    }

    fn run_shell_operation(
        &mut self,
        kind: ShellOperationKind,
        sources: Vec<PathBuf>,
        target: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if sources.is_empty() {
            return;
        }
        let count = sources.len();
        // 删除走回收站，可 Ctrl+Z 恢复
        if matches!(kind, ShellOperationKind::Delete) {
            self.push_undo(UndoEntry::Restore(sources.clone()));
        }
        self.status = format!("正在{} {} 项", kind.progress_label(), count);
        self.left_selected.clear();
        self.right_selected.clear();
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { perform_shell_file_operation(kind, &sources, target.as_deref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            wait_out_of_ole_modal(cx).await;
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(()) => format!("已{} {} 项", kind.progress_label(), count),
                    Err(error) => format!("{}失败：{}", kind.progress_label(), error),
                };
                this.reload_visible_async(cx);
            });
        })
        .detach();
    }

    fn save_config(&self) {
        let path = config_path();
        let mut value = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let object = value
            .as_object_mut()
            .expect("configuration root must be an object");
        object.insert("split".into(), self.split.into());
        object.insert(
            "view_mode".into(),
            self.left_view_mode.label().replace(' ', "").into(),
        );
        object.insert(
            "left_path".into(),
            self.current_path().display().to_string().into(),
        );
        object.insert(
            "right_path".into(),
            self.right_path.display().to_string().into(),
        );
        object.insert("show_hidden".into(), self.show_hidden.into());
        object.insert("show_sidebar".into(), self.show_sidebar.into());
        object.insert("ui_font_size".into(), serde_json::json!(self.ui_font_size));
        object.insert(
            "file_font_size".into(),
            serde_json::json!(self.file_font_size),
        );
        object.insert("row_spacing".into(), serde_json::json!(self.row_spacing));
        object.insert("load_thumbnails".into(), self.load_thumbnails.into());
        object.insert("win_e_enabled".into(), self.win_e_enabled.into());
        object.insert(
            "explorer_replacement".into(),
            self.explorer_replacement.into(),
        );
        object.insert(
            "shortcut_bindings".into(),
            serde_json::to_value(&self.shortcut_bindings).unwrap_or_default(),
        );
        object.insert(
            "favorites".into(),
            serde_json::Value::Array(
                self.favorites
                    .iter()
                    .map(|path| serde_json::Value::String(path.display().to_string()))
                    .collect(),
            ),
        );
        object.insert(
            "recents".into(),
            serde_json::Value::Array(
                self.recents
                    .iter()
                    .map(|path| serde_json::Value::String(path.display().to_string()))
                    .collect(),
            ),
        );
        object.insert("recent_limit".into(), serde_json::json!(self.recent_limit));
        object.insert("shell_menu_default".into(), self.shell_menu_default.into());
        object.insert(
            "shell_menu_third_party".into(),
            self.shell_menu_third_party.into(),
        );
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            path,
            serde_json::to_string_pretty(&value).unwrap_or_default(),
        );
    }

    fn window_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let native_button = |id: &'static str, label: &'static str, area: WindowControlArea| {
            div()
                .id(id)
                .w(px(46.))
                .h(px(34.))
                .mb(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .window_control_area(area)
                .hover(move |style| {
                    style
                        .bg(if area == WindowControlArea::Close {
                            rgb(0xe81123)
                        } else {
                            rgb(0xdde3e6)
                        })
                        .text_color(if area == WindowControlArea::Close {
                            rgb(0xffffff)
                        } else {
                            rgb(TEXT)
                        })
                })
                .child(label)
        };
        let app_button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .w(px(46.))
                .h(px(34.))
                .mb(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .hover(|style| style.bg(rgb(0xdde3e6)).text_color(rgb(TEXT)))
                .child(label)
        };
        div()
            .flex()
            .items_center()
            .child(
                native_button("win-min", "−", WindowControlArea::Min)
                    .on_click(cx.listener(|_, _, window, _| window.minimize_window())),
            )
            .child(
                app_button("win-max", "□")
                    .on_click(cx.listener(|_, _, window, _| toggle_native_maximize(window))),
            )
            .child(
                native_button("win-close", "×", WindowControlArea::Close)
                    .on_click(cx.listener(|_, _, _, _| hide_fileflow_to_tray())),
            )
    }

    fn tab_bar(&self, show_window_buttons: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tabs.iter().enumerate().map(|(index, path)| {
            let label = truncate(&tab_name(path), 18);
            let active = index == self.active_tab;
            div()
                .id(("tab", index))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_t_md()
                .bg(if active { rgb(0xffffff) } else { rgb(0xf0f2f4) })
                .text_color(if active { rgb(TEXT) } else { rgb(MUTED) })
                .child(folder_icon(16.))
                .child(label)
                .child(
                    div()
                        .id(("close", index))
                        .text_color(rgb(MUTED))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_tab(index, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_tab = index;
                    let path = this.current_path().to_path_buf();
                    this.active_side = "left";
                    this.load_side_async("left", path, cx);
                }))
        });
        div()
            .h(px(42.))
            .pl_3()
            .flex()
            .items_end()
            .gap_1()
            .bg(rgb(0xf0f2f4))
            .child(
                div()
                    .id("open-settings")
                    .pb_2()
                    .text_color(rgb(BLUE))
                    .child("☰")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.on_open_settings(&OpenSettings, window, cx)
                    })),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(rgb(0xd9dfe3)).mx_2())
            .children(tabs)
            .child(
                div()
                    .id("new-tab")
                    .px_3()
                    .py_2()
                    .mb(px(1.))
                    .text_color(rgb(MUTED))
                    .child("＋")
                    .on_click(cx.listener(|this, _, _, cx| this.add_tab(cx))),
            )
            .child(
                div()
                    .id("tabbar-drag-left")
                    .flex_1()
                    .h_full()
                    .min_h(px(42.))
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .id("split")
                    .px_2()
                    .py_2()
                    .mb(px(1.))
                    .text_color(rgb(if self.split { BLUE } else { MUTED }))
                    .child("▥")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_split(cx))),
            )
            .when(show_window_buttons, |bar| {
                bar.child(self.window_buttons(cx))
            })
    }

    fn right_tab_bar(&self, show_window_buttons: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.right_tabs.iter().enumerate().map(|(index, path)| {
            let label = truncate(&tab_name(path), 18);
            let active = index == self.right_active_tab;
            div()
                .id(("right-tab", index))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_t_md()
                .bg(if active { rgb(0xffffff) } else { rgb(0xf0f2f4) })
                .text_color(if active { rgb(TEXT) } else { rgb(MUTED) })
                .child(folder_icon(16.))
                .child(label)
                .child(
                    div()
                        .id(("right-tab-close", index))
                        .text_color(rgb(MUTED))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_right_tab(index, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.right_active_tab = index;
                    this.right_path = this.right_tabs[index].clone();
                    this.active_side = "right";
                    this.load_side_async("right", this.right_path.clone(), cx);
                }))
        });
        div()
            .h(px(42.))
            .pl_2()
            .flex()
            .items_end()
            .gap_1()
            .bg(rgb(0xf0f2f4))
            .border_b_1()
            .border_color(rgb(0xd9dfe3))
            .children(tabs)
            .child(
                div()
                    .id("new-right-tab")
                    .px_3()
                    .py_2()
                    .mb(px(1.))
                    .text_color(rgb(MUTED))
                    .child("＋")
                    .on_click(cx.listener(|this, _, _, cx| this.add_right_tab(cx))),
            )
            .child(
                div()
                    .id("tabbar-drag-right")
                    .flex_1()
                    .h_full()
                    .min_h(px(42.))
                    .window_control_area(WindowControlArea::Drag),
            )
            .when(show_window_buttons, |bar| {
                bar.child(self.window_buttons(cx))
            })
    }

    fn pane_header(
        &self,
        path: &Path,
        side: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let crumbs = breadcrumb_paths(path);
        let crumb_count = crumbs.len();
        let total_crumb_chars = crumbs
            .iter()
            .map(|(label, _)| label.chars().count())
            .sum::<usize>();
        let compress_crumbs = self.split || total_crumb_chars > 70 || crumb_count > 8;
        let forward_crumbs = if side == "left" {
            forward_breadcrumb_paths(path, &self.left_forward)
        } else {
            forward_breadcrumb_paths(path, &self.right_forward)
        };
        let current_folder = path.to_path_buf();
        let is_favorite = self
            .favorites
            .iter()
            .any(|favorite| favorite == &current_folder);
        let nav_button = |id: &'static str, icon: &'static str| {
            div()
                .id(id)
                .w(px(34.))
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .hover(|style| style.bg(rgb(HOVER_BLUE)).text_color(rgb(TEXT)))
                .child(img(nav_icon_path(icon)).w(px(22.)).h(px(22.)))
        };
        div()
            .relative()
            .h(px(40.))
            .flex_none()
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .border_b_1()
            .border_color(rgb(0xe2e7ea))
            .child(
                nav_button(
                    if side == "left" {
                        "back-left"
                    } else {
                        "back-right"
                    },
                    "back",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_side = side;
                    this.go_back(side, cx);
                })),
            )
            .child(
                nav_button(
                    if side == "left" {
                        "forward-left"
                    } else {
                        "forward-right"
                    },
                    "forward",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_side = side;
                    this.go_forward(side, cx);
                })),
            )
            .child(
                nav_button(
                    if side == "left" {
                        "up-left"
                    } else {
                        "up-right"
                    },
                    "up",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_side = side;
                    this.go_up_side(side, cx);
                })),
            )
            .child(div().w(px(1.)).h(px(24.)).mx_2().bg(rgb(0xd9dfe3)))
            .child(
                nav_button(
                    if side == "left" {
                        "favorite-left"
                    } else {
                        "favorite-right"
                    },
                    if is_favorite {
                        "bookmark-filled"
                    } else {
                        "bookmark"
                    },
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.active_side = side;
                    this.set_single_selection(side, current_folder.clone());
                    this.on_toggle_favorite(&ToggleFavorite, window, cx);
                })),
            )
            .child(div().w(px(1.)).h(px(24.)).mx_2().bg(rgb(0xd9dfe3)))
            .when(
                self.address_editing && self.address_side == side,
                |header| {
                    header.child(
                        div()
                            .id(if side == "left" {
                                "address-input-frame-left"
                            } else {
                                "address-input-frame-right"
                            })
                            .flex_1()
                            .h(px(28.))
                            .px_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(BLUE))
                            .child(self.address_input.clone()),
                    )
                },
            )
            .when(
                !(self.address_editing && self.address_side == side),
                |header| {
                    header.child(
                        div()
                            .id(if side == "left" {
                                "crumb-strip-left"
                            } else {
                                "crumb-strip-right"
                            })
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(crumbs.into_iter().enumerate().map(
                                |(index, (crumb, crumb_path))| {
                                    let is_last = index + 1 == crumb_count;
                                    let limit = if compress_crumbs {
                                        if is_last { 28 } else { 14 }
                                    } else {
                                        usize::MAX
                                    };
                                    let label = ellipsis_middle(&crumb, limit);
                                    let width = crumb_width_for(&label, compress_crumbs);
                                    div()
                                        .id((
                                            if side == "left" {
                                                "crumb-left"
                                            } else {
                                                "crumb-right"
                                            },
                                            index,
                                        ))
                                        .h(px(30.))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .text_color(rgb(if is_last { TEXT } else { MUTED }))
                                        .child(
                                            div()
                                                .id((
                                                    if side == "left" {
                                                        "crumb-chip-left"
                                                    } else {
                                                        "crumb-chip-right"
                                                    },
                                                    index,
                                                ))
                                                .flex_none()
                                                .w(px(width))
                                                .h(px(28.))
                                                .px_2()
                                                .rounded_md()
                                                .overflow_hidden()
                                                .flex()
                                                .items_center()
                                                .hover(|style| {
                                                    style.bg(rgb(HOVER_BLUE)).text_color(rgb(TEXT))
                                                })
                                                .tooltip({
                                                    let full = crumb_path.display().to_string();
                                                    move |_, cx| {
                                                        cx.new(|_| CrumbTooltip {
                                                            text: full.clone(),
                                                        })
                                                        .into()
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .h(px(22.))
                                                        .overflow_hidden()
                                                        .truncate()
                                                        .flex()
                                                        .items_center()
                                                        .child(label),
                                                ),
                                        )
                                        .child(if is_last { "" } else { "›" })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if is_virtual_path(&crumb_path) {
                                                this.open_virtual(side, crumb_path.clone(), cx);
                                            } else if side == "left" {
                                                this.navigate_left(crumb_path.clone(), cx);
                                            } else {
                                                this.navigate_right(crumb_path.clone(), cx);
                                            }
                                        }))
                                },
                            ))
                            .children(forward_crumbs.into_iter().enumerate().map(
                                |(index, (crumb, crumb_path))| {
                                    let label = ellipsis_middle(&crumb, 7);
                                    let width = crumb_width_for(&label, true);
                                    div()
                                        .id((
                                            if side == "left" {
                                                "forward-crumb-left"
                                            } else {
                                                "forward-crumb-right"
                                            },
                                            index,
                                        ))
                                        .w(px(width))
                                        .h(px(24.))
                                        .overflow_hidden()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_color(rgb(0xa7afb5))
                                        .hover(|style| style.text_color(rgb(BLUE)))
                                        .child("›")
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .h(px(22.))
                                                .overflow_hidden()
                                                .truncate()
                                                .flex()
                                                .items_center()
                                                .child(label),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if side == "left" {
                                                this.left_back
                                                    .push(this.current_path().to_path_buf());
                                                this.left_forward.clear();
                                                this.navigate_left_history(crumb_path.clone(), cx);
                                            } else {
                                                this.right_back.push(this.right_path.clone());
                                                this.right_forward.clear();
                                                this.navigate_right_history(crumb_path.clone(), cx);
                                            }
                                        }))
                                },
                            ))
                            .child(
                                div()
                                    .id(if side == "left" {
                                        "address-edit-left"
                                    } else {
                                        "address-edit-right"
                                    })
                                    .flex_1()
                                    .min_w(px(24.))
                                    .h_full()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.focus_address_for_side(side, window, cx)
                                    })),
                            ),
                    )
                },
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let home = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let locations = [
            ("主页", home.clone()),
            ("桌面", home.join("Desktop")),
            ("下载", home.join("Downloads")),
            ("文档", home.join("Documents")),
            ("图片", home.join("Pictures")),
            ("视频", home.join("Videos")),
            ("此电脑", PathBuf::from(THIS_PC_PATH)),
            ("回收站", PathBuf::from(RECYCLE_BIN_PATH)),
        ];
        let drives = available_drives();
        let favorites = self.favorites.clone();
        let recents = self.recents.clone();
        div()
            .w(px(218.))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf8fafb))
            .border_r_1()
            .border_color(rgb(0xd9dfe3))
            .child(
                div()
                    .h(px(36.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xe0e5e8))
                    .text_color(rgb(MUTED))
                    .child("⌕  筛选位置"),
            )
            .when(!recents.is_empty(), |bar| {
                bar.child(
                    div()
                        .h(px(32.))
                        .px_3()
                        .pt_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_color(rgb(MUTED))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child("◷")
                                .child("Recents"),
                        )
                        .child("⌄"),
                )
                .children(recents.into_iter().enumerate().map(|(index, path)| {
                    let label = truncate(&tab_name(&path), 18);
                    // 渲染零 IO：缓存未命中后台 stat，先按扩展名猜图标
                    let is_dir = cached_path_is_dir(&path, path.extension().is_none());
                    div()
                        .id(("recent", index))
                        .h(px(31.))
                        .mx_2()
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_sm()
                        .hover(|style| style.bg(rgb(HOVER_BLUE)))
                        .text_color(rgb(TEXT))
                        .child(if is_dir {
                            folder_icon(15.)
                        } else {
                            inline_file_icon(&path, 15.)
                        })
                        .child(div().flex_1().min_w(px(0.)).truncate().child(label))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dismiss_popups_except_new_folder();
                            if can_open_directory(&path) {
                                this.navigate_active(path.clone(), cx);
                            } else if path.exists() {
                                this.remember_recent(&path);
                                this.save_config();
                                let opened = shell_open_default(&path);
                                this.status = if opened {
                                    "已用系统默认程序打开"
                                } else {
                                    "系统默认程序打开失败"
                                }
                                .to_string();
                                cx.notify();
                            }
                        }))
                }))
                .child(div().mx_3().mt_2().h(px(1.)).bg(rgb(0xd9dfe3)))
            })
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Places"),
            )
            .children(
                locations
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, path))| {
                        // 渲染零 IO：位置基本固定，默认当作存在，后台校验兜底
                        let exists = is_virtual_path(&path) || cached_path_is_dir(&path, true);
                        div()
                            .id(("place", index))
                            .h(px(31.))
                            .mx_2()
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded_sm()
                            .hover(|style| style.bg(rgb(HOVER_BLUE)))
                            .text_color(rgb(if exists { TEXT } else { MUTED }))
                            .child(folder_icon(15.))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_popups_except_new_folder();
                                if is_virtual_path(&path) {
                                    this.open_virtual_active(path.clone(), cx);
                                } else if path.is_dir() {
                                    this.navigate_active(path.clone(), cx);
                                }
                            }))
                    }),
            )
            .when(!favorites.is_empty(), |bar| {
                bar.child(div().mx_3().mt_3().h(px(1.)).bg(rgb(0xd9dfe3)))
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("收藏"),
                    )
                    .children(favorites.into_iter().enumerate().map(|(index, path)| {
                        let label = tab_name(&path);
                        div()
                            .id(("favorite", index))
                            .h(px(31.))
                            .mx_2()
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .hover(|style| style.bg(rgb(HOVER_BLUE)))
                            .child(folder_icon(15.))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_popups_except_new_folder();
                                this.navigate_active(path.clone(), cx);
                            }))
                    }))
            })
            .child(div().mx_3().mt_3().h(px(1.)).bg(rgb(0xd9dfe3)))
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Storage"),
            )
            .children(drives.into_iter().enumerate().map(|(index, path)| {
                let label = format!("{}", path.display());
                div()
                    .id(("drive", index))
                    .h(px(31.))
                    .mx_2()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .hover(|style| style.bg(rgb(HOVER_BLUE)))
                    .child(folder_icon(15.))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dismiss_popups_except_new_folder();
                        this.navigate_active(path.clone(), cx);
                    }))
            }))
    }

    fn entries_view(
        &self,
        entries: &[Entry],
        side: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_mode = self.view_mode_for(side);
        let scroll_handle = if side == "left" {
            self.left_scroll_handle.clone()
        } else {
            self.right_scroll_handle.clone()
        };
        // grid/columns 仍走普通 ScrollHandle（base_handle）；
        // List/Details 走 UniformListScrollHandle 虚拟化（track_scroll 消费整个 handle）
        let base_scroll_handle = scroll_handle.0.borrow().base_handle.clone();
        let visible_paths: Vec<PathBuf> = entries.iter().map(|entry| entry.path.clone()).collect();
        let icon_mode = matches!(
            view_mode,
            ViewMode::MIcons | ViewMode::LIcons | ViewMode::XLIcons
        );
        let tile = match view_mode {
            ViewMode::MIcons => 110.,
            ViewMode::LIcons => 160.,
            ViewMode::XLIcons => 220.,
            _ => 0.,
        };
        let list_row_h =
            (self.file_font_size * 1.42 + self.row_spacing).max(27. + self.row_spacing);
        // Details 列宽单一来源：与 entry_list_row / blank_details_cell 一致
        // Details 列宽单一来源：details_col_widths()（表头 / 数据行 / 空白单元格共用）
        let [details_type_w, details_date_w, details_size_w] = self.details_col_widths();
        let body = if icon_mode {
            // 图标视图（M/L/XL）：uniform_list 虚拟化，每个虚拟行 = 一横条 cols 个图块。
            // 布局公式单一来源：virtual_grid_geometry（渲染/拖框/拖放/键盘导航共用），
            // 列数按滚动容器实际宽度计算（窗口缩放时自动重排列）。
            let view = cx.entity();
            let geo_view = cx.entity();
            let font_size = self.file_font_size;
            let row_spacing = self.row_spacing;
            let load_thumbnails = self.load_thumbnails;
            let split = self.split;
            let view_mode_captured = view_mode;
            let tile_captured = tile;
            let visible_paths_for_rows = visible_paths.clone();
            let base_for_cols = base_scroll_handle.clone();
            let geo = virtual_grid_geometry(
                view_mode,
                font_size,
                row_spacing,
                f32::from(base_for_cols.bounds().size.width).max(1.),
                entries.len(),
            );
            let cols = geo.cols.max(1);
            let row_count = geo.rows();
            let cell_h = geo.cell_h.ceil();
            let (sort_col, sort_asc) = self.sort_state_for(side);
            let data_epoch = (sort_col as u8) << 1 | sort_asc as u8;
            let list = uniform_list(
                (
                    if side == "left" {
                        "vgrid-left"
                    } else {
                        "vgrid-right"
                    },
                    data_epoch as usize,
                ),
                row_count,
                move |ix, _window, cx| {
                    let mut rows = Vec::with_capacity(ix.len());
                    for row in ix {
                        // 每行构建 cols 个单元格（最后一行可能不满）
                        let mut cells = Vec::with_capacity(cols);
                        for col in 0..cols {
                            let index = row * cols + col;
                            let Some(entry) = geo_view.update(cx, |this, _| {
                                let entries = if side == "left" {
                                    &this.left_display_entries
                                } else {
                                    &this.right_display_entries
                                };
                                entries.get(index).cloned()
                            }) else {
                                continue;
                            };
                            cells.push(geo_view.update(cx, |this, cx| {
                                this.entry_grid_cell(
                                    side,
                                    index,
                                    &entry,
                                    visible_paths_for_rows.clone(),
                                    view_mode_captured,
                                    tile_captured,
                                    font_size,
                                    load_thumbnails,
                                    cx,
                                )
                            }));
                        }
                        if cells.is_empty() {
                            continue;
                        }
                        rows.push(
                            div()
                                .id((
                                    if side == "left" {
                                        "vgrid-row-left"
                                    } else {
                                        "vgrid-row-right"
                                    },
                                    row,
                                ))
                                .h(px(cell_h))
                                .w_full()
                                .flex_none()
                                .flex()
                                .gap_2()
                                .children(cells),
                        );
                    }
                    rows
                },
            )
            .track_scroll(scroll_handle.clone())
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .p_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let visible_paths = visible_paths.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        this.begin_drag_select(
                            side,
                            event.position,
                            visible_paths.clone(),
                            event.modifiers,
                            cx,
                        );
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_file_drag(event, window, cx);
                this.update_drag_select(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if !this.finish_file_drag(event, window, cx) {
                        this.finish_drag_select(event, window, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            );
            let _ = (view, split);
            list.into_any_element()
        } else if view_mode == ViewMode::Columns {
            // 分栏视图：uniform_list 虚拟化，每个虚拟行 = 一横条 cols 个条目。
            let view = cx.entity();
            let font_size = self.file_font_size;
            let row_spacing = self.row_spacing;
            let view_mode_captured = view_mode;
            let visible_paths_for_rows = visible_paths.clone();
            let base_for_cols = base_scroll_handle.clone();
            let pane_w = f32::from(base_for_cols.bounds().size.width).max(1.);
            let item_w = 240.;
            let cell_h = ((font_size * 1.5 + row_spacing).max(28. + row_spacing)).ceil();
            let cols = (((pane_w - 16.0) / item_w).floor() as usize).max(1);
            let row_count = entries.len().div_ceil(cols);
            let (sort_col, sort_asc) = self.sort_state_for(side);
            let data_epoch = (sort_col as u8) << 1 | sort_asc as u8;
            let list = uniform_list(
                (
                    if side == "left" {
                        "vcolumns-left"
                    } else {
                        "vcolumns-right"
                    },
                    data_epoch as usize,
                ),
                row_count,
                move |ix, _window, cx| {
                    let mut rows = Vec::with_capacity(ix.len());
                    for row in ix {
                        let mut cells = Vec::with_capacity(cols);
                        for col in 0..cols {
                            let index = row * cols + col;
                            let Some(entry) = view.update(cx, |this, _| {
                                let entries = if side == "left" {
                                    &this.left_display_entries
                                } else {
                                    &this.right_display_entries
                                };
                                entries.get(index).cloned()
                            }) else {
                                continue;
                            };
                            cells.push(view.update(cx, |this, cx| {
                                this.entry_column_cell(
                                    side,
                                    index,
                                    &entry,
                                    visible_paths_for_rows.clone(),
                                    view_mode_captured,
                                    item_w,
                                    cell_h,
                                    font_size,
                                    cx,
                                )
                            }));
                        }
                        if cells.is_empty() {
                            continue;
                        }
                        rows.push(
                            div()
                                .id((
                                    if side == "left" {
                                        "vcolumns-row-left"
                                    } else {
                                        "vcolumns-row-right"
                                    },
                                    row,
                                ))
                                .h(px(cell_h))
                                .w_full()
                                .flex_none()
                                .flex()
                                .gap_2()
                                .children(cells),
                        );
                    }
                    rows
                },
            )
            .track_scroll(scroll_handle.clone())
            .flex_1()
            .size_full()
            .min_w(px(0.))
            .p_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let visible_paths = visible_paths.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        this.begin_drag_select(
                            side,
                            event.position,
                            visible_paths.clone(),
                            event.modifiers,
                            cx,
                        );
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_file_drag(event, window, cx);
                this.update_drag_select(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if !this.finish_file_drag(event, window, cx) {
                        this.finish_drag_select(event, window, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            );
            list.into_any_element()
        } else {
            // List / Details 视图：uniform_list 虚拟化渲染。
            // 大目录（如 900+ 项的 Downloads）全量构建每项 ~15 个元素会卡顿掉帧，
            // uniform_list 只构建可视行（~30 行），滚动/渲染成本 O(可视区)。
            // 拖框选择与拖放命中由 apply_drag_selection/file_drop_target 的
            // 行高×index 数学推算路径承担（uniform_list 不填充 child_bounds）。
            let view = cx.entity();
            let row_height = list_row_h.ceil();
            let view_mode_captured = view_mode;
            let visible_paths_for_rows = visible_paths.clone();
            // 数据版本：排序/方向变化时 id 变化，强制 uniform_list 重建行
            // （否则行复用缓存不感知顺序变化，列表"看起来没刷新"）
            let (sort_col, sort_asc) = self.sort_state_for(side);
            let data_epoch = (sort_col as u8) << 1 | sort_asc as u8;
            let list = uniform_list(
                (
                    if side == "left" {
                        "vlist-left"
                    } else {
                        "vlist-right"
                    },
                    data_epoch as usize,
                ),
                entries.len(),
                move |ix, _window, cx| {
                    let mut rows = Vec::with_capacity(ix.len());
                    for index in ix {
                        let Some(entry) = view.update(cx, |this, _| {
                            let entries = if side == "left" {
                                &this.left_display_entries
                            } else {
                                &this.right_display_entries
                            };
                            entries.get(index).cloned()
                        }) else {
                            continue;
                        };
                        rows.push(view.update(cx, |this, cx| {
                            this.entry_list_row(
                                side,
                                index,
                                &entry,
                                visible_paths_for_rows.clone(),
                                view_mode_captured,
                                row_height,
                                cx,
                            )
                        }));
                    }
                    rows
                },
            )
            .track_scroll(scroll_handle.clone())
            .flex_1()
            .min_w(px(0.));
            div()
                .id(if side == "left" {
                    "list-left"
                } else {
                    "list-right"
                })
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        let visible_paths = visible_paths.clone();
                        move |this, event: &MouseDownEvent, _, cx| {
                            this.begin_drag_select(
                                side,
                                event.position,
                                visible_paths.clone(),
                                event.modifiers,
                                cx,
                            );
                        }
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                    this.update_file_drag(event, window, cx);
                    this.update_drag_select(event, window, cx);
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseUpEvent, window, cx| {
                        if !this.finish_file_drag(event, window, cx) {
                            this.finish_drag_select(event, window, cx);
                        }
                    }),
                )

                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.on_blank_area_mouse_down(side, event, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Back),
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.on_blank_area_mouse_down(side, event, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Forward),
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.on_blank_area_mouse_down(side, event, cx);
                    }),
                )
                .child(
                    div()
                        .h(px(30.))
                        .flex_none()
                        .relative()
                        .pr_3()
                        .border_b_1()
                        .border_color(rgb(0xd9dfe3))
                        .text_color(rgb(MUTED))
                        .text_sm()
                        .min_w(px(0.))
                        .overflow_hidden()
                        // 30px 占位已含在 pl(30) 内：行结构是 pl(30)+图标(15)+gap(12)，
                        // 表头不重复占位，名称/类型/日期/大小列才能与数据上下对齐
                        .child(
                            div()
                                .id(if side == "left" {
                                    "sort-name-left"
                                } else {
                                    "sort-name-right"
                                })
                                .absolute()
                                .left(px(30.))
                                .right(px(details_type_w + details_date_w + details_size_w + 36.0))
                                .top(px(0.))
                                .bottom(px(0.))
                                .flex()
                                .items_center()
                                .overflow_hidden()
                                .hover(|style| style.text_color(rgb(BLUE)))
                                .child(sort_header_label(
                                    "名称",
                                    self.sort_state_for(side).0 == SortColumn::Name,
                                    self.sort_state_for(side).1,
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.sort_by(side, SortColumn::Name, cx)
                                })),
                        )
                        .when(view_mode == ViewMode::Details, |header| {
                            header
                                .child(
                                    div()
                                        .id(if side == "left" {
                                            "sort-type-left"
                                        } else {
                                            "sort-type-right"
                                        })
                                        .absolute()
                                        .right(px(details_size_w + details_date_w + 24.0))
                                        .top(px(0.))
                                        .bottom(px(0.))
                                        .w(px(details_type_w))
                                        .overflow_hidden()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .hover(|style| style.text_color(rgb(BLUE)))
                                        .child(div().truncate().child(sort_header_label(
                                            "类型",
                                            self.sort_state_for(side).0 == SortColumn::Type,
                                            self.sort_state_for(side).1,
                                        )))
                                        .child(
                                            div()
                                                .id(if side == "left" {
                                                    "type-filter-left"
                                                } else {
                                                    "type-filter-right"
                                                })
                                                .px_1()
                                                .text_color(rgb(
                                                    if self.type_filter_allowed.is_empty() {
                                                        MUTED
                                                    } else {
                                                        BLUE
                                                    },
                                                ))
                                                .child("▽")
                                                .on_click(cx.listener(
                                                    move |this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        this.open_type_filter(side, cx);
                                                    },
                                                )),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.sort_by(side, SortColumn::Type, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id(if side == "left" {
                                            "sort-date-left"
                                        } else {
                                            "sort-date-right"
                                        })
                                        .absolute()
                                        .right(px(details_size_w + 12.0))
                                        .top(px(0.))
                                        .bottom(px(0.))
                                        .w(px(details_date_w))
                                        .overflow_hidden()
                                        .truncate()
                                        .hover(|style| style.text_color(rgb(BLUE)))
                                        .child(sort_header_label(
                                            "修改日期",
                                            self.sort_state_for(side).0 == SortColumn::Modified,
                                            self.sort_state_for(side).1,
                                        ))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.sort_by(side, SortColumn::Modified, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id(if side == "left" {
                                            "sort-size-left"
                                        } else {
                                            "sort-size-right"
                                        })
                                        .absolute()
                                        .right(px(0.))
                                        .top(px(0.))
                                        .bottom(px(0.))
                                        .w(px(details_size_w))
                                        .overflow_hidden()
                                        // 左对齐必须用 flex + justify_start：
                                        // 本单元格是 .flex() 容器，text_right()/
                                        // truncate() 都只对匿名文本块生效，对 .child()
                                        // 子元素无效 → "大小"会贴到列右缘，
                                        // 与下方左对齐的数值不在一条竖线上。
                                        .flex()
                                        .items_center()
                                        .justify_start()
                                        .child(
                                            div()
                                                .min_w(px(0.))
                                                .overflow_hidden()
                                                .truncate()
                                                .hover(|style| style.text_color(rgb(BLUE)))
                                                .child(sort_header_label(
                                                    "大小",
                                                    self.sort_state_for(side).0 == SortColumn::Size,
                                                    self.sort_state_for(side).1,
                                                )),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.sort_by(side, SortColumn::Size, cx)
                                        })),
                                )
                        }),
                )
                .child(list)
                .into_any_element()
        };
        div()
            .id(if side == "left" {
                "pane-hit-left"
            } else {
                "pane-hit-right"
            })
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // 所有视图共用空白点击入口；条目自身的点击处理会停止冒泡。
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.on_blank_area_click(side, event, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_drop(cx.listener(move |this, payload: &FileDragPayload, _, cx| {
                let target = if side == "right" && this.split {
                    this.right_path.clone()
                } else {
                    this.current_path().to_path_buf()
                };
                this.drop_files_to(&payload.sources, &target, false, cx);
            }))
            .child(body)
    }

    /// List / Details 视图的单行渲染（uniform_list 虚拟化后按需构建）。
    /// 行高由调用方锚定（row_height），内部不再自适应。
    #[allow(clippy::too_many_arguments)]
    fn entry_list_row(
        &self,
        side: &'static str,
        index: usize,
        entry: &Entry,
        visible_paths: Vec<PathBuf>,
        view_mode: ViewMode,
        row_height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = entry.path.clone();
        let menu_path = entry.path.clone();
        let is_dir = entry.is_dir;
        let name = entry.name.clone();
        let size = entry.size;
        let modified = entry.modified.clone();
        let row_menu_path = entry.path.clone();
        let details_menu_path = entry.path.clone();
        let selected = self.is_selected(side, &path);
        let focused = self.is_focused_selection(side, &path);
        let drag_sources: Vec<PathBuf> = if selected {
            if side == "left" {
                self.left_selected.iter().cloned().collect()
            } else {
                self.right_selected.iter().cloned().collect()
            }
        } else {
            vec![path.clone()]
        };
        let drag_label = if drag_sources.len() > 1 {
            format!("{} 项", drag_sources.len())
        } else {
            name.clone()
        };
        let drop_target = path.clone();
        let size_dir_path = if is_dir { Some(path.clone()) } else { None };
        // Details 固定像素列：类型/日期/大小从行尾锚定，
        // 名称占剩余宽度——无论文件名多长，三列 x 坐标恒定。
        // 列宽单一来源：details_col_widths()（与表头严格一致）。
        let [details_type_w, details_date_w, details_size_w] = self.details_col_widths();
        let is_details = view_mode == ViewMode::Details;
        div()
            .id((
                if side == "left" {
                    "entry-list-left"
                } else {
                    "entry-list-right"
                },
                index,
            ))
            .h(px(row_height))
            .w_full()
            .flex_none()
            .relative()
            .pr_3()
            .border_b_1()
            .border_color(rgb(0xf0f2f3))
            .text_size(px(self.file_font_size))
            .min_w(px(0.))
            .overflow_hidden()
            .bg(if selected {
                rgb(if focused { BLUE } else { HOVER_BLUE })
            } else {
                rgb(0xffffff)
            })
            .hover(move |style| style.bg(rgb(if selected { BLUE } else { HOVER_BLUE })))
            .when(is_dir, |row| {
                row.on_drop(cx.listener(
                    move |this, payload: &FileDragPayload, _, cx| {
                        this.drop_files_to(&payload.sources, &drop_target, false, cx);
                    },
                ))
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if !event.is_right_click() {
                    this.on_blank_area_click(side, event, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.cancel_drag_select();
                    this.begin_context_target(
                        side,
                        row_menu_path.clone(),
                        event.position,
                        cx,
                    );
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    // 真实路径的合并 Shell 菜单已在 mouse_down 弹出，这里只补虚拟路径 pending
                    this.finish_context_target(event.position, cx);
                }),
            )
            .child(
                div()
                    .id((
                        if side == "left" {
                            "entry-name-left"
                        } else {
                            "entry-name-right"
                        },
                        index,
                    ))
                    .absolute()
                    .left(px(30.))
                    .right(px(if is_details {
                        details_type_w + details_date_w + details_size_w + 36.0
                    } else {
                        0.0
                    }))
                    .top(px(0.))
                    .bottom(px(0.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .overflow_hidden()
                    .cursor_move()
                    .on_drag(
                        FileDragPayload {
                            sources: drag_sources.clone(),
                        },
                        move |_payload: &FileDragPayload, position, _, cx| {
                            cx.new(|_| FileDragPreview {
                                label: drag_label.clone(),
                                position,
                            })
                        },
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let drag_path = path.clone();
                            let drag_visible_paths = visible_paths.clone();
                            move |this, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                if event.modifiers.control || event.modifiers.shift {
                                    this.select_entry(
                                        side,
                                        drag_path.clone(),
                                        &drag_visible_paths,
                                        event.modifiers,
                                        cx,
                                    );
                                    this.file_drag = None;
                                } else {
                                    this.begin_file_drag(
                                        side,
                                        drag_path.clone(),
                                        &drag_visible_paths,
                                        event.position,
                                        event.modifiers,
                                        cx,
                                    );
                                }
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(
                        |this, event: &MouseMoveEvent, window, cx| {
                            cx.stop_propagation();
                            this.update_file_drag(event, window, cx);
                            this.update_drag_select(event, window, cx);
                        },
                    ))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, window, cx| {
                            cx.stop_propagation();
                            if !this.finish_file_drag(event, window, cx) {
                                this.finish_drag_select(event, window, cx);
                            }
                        }),
                    )
                    .child(if is_dir {
                        folder_icon(17.)
                    } else {
                        inline_file_icon(&path, 15.)
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_color(if selected && focused {
                                rgb(0xffffff)
                            } else {
                                rgb(TEXT)
                            })
                            .truncate()
                            .child(name),
                    )
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if this.suppress_blank_click {
                            this.suppress_blank_click = false;
                            cx.notify();
                            return;
                        }
                        if event.is_right_click() {
                            return;
                        }
                        if event.modifiers().control || event.modifiers().shift {
                            return;
                        }
                        this.dismiss_popups_except_new_folder();
                        if event.click_count() >= 2 {
                            this.set_single_selection(side, path.clone());
                            this.open_selected(cx);
                        } else {
                            this.select_entry(
                                side,
                                path.clone(),
                                &visible_paths,
                                event.modifiers(),
                                cx,
                            );
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.cancel_drag_select();
                            this.begin_context_target(
                                side,
                                menu_path.clone(),
                                event.position,
                                cx,
                            );
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                            cx.stop_propagation();
                            // 真实路径的合并 Shell 菜单已在 mouse_down 弹出，这里只补虚拟路径 pending
                            this.finish_context_target(event.position, cx);
                        }),
                    ),
            )
            .when(view_mode == ViewMode::Details, |row| {
                row.child(self.blank_details_cell(
                    side,
                    index * 3,
                    details_type_w,
                    details_size_w + details_date_w + 24.0,
                    if is_dir {
                        "文件夹".to_string()
                    } else {
                        "文件".to_string()
                    },
                    selected,
                    focused,
                    false,
                    details_menu_path.clone(),
                    cx,
                ))
                .child(self.blank_details_cell(
                    side,
                    index * 3 + 1,
                    details_date_w,
                    details_size_w + 12.0,
                    modified,
                    selected,
                    focused,
                    false,
                    details_menu_path.clone(),
                    cx,
                ))
                .child(self.blank_details_cell(
                    side,
                    index * 3 + 2,
                    details_size_w,
                    0.0,
                    if is_dir {
                        // 文件夹：后台递归计算总大小（缓存命中才显示，未算完留空，
                        // 不阻塞渲染；算完经 FOLDER_SIZE_DONE 泵节流刷新）
                        size_dir_path
                            .as_deref()
                            .and_then(folder_size_or_request)
                            .map(format_size)
                            .unwrap_or_default()
                    } else {
                        format_size(size)
                    },
                    selected,
                    focused,
                    true,
                    details_menu_path.clone(),
                    cx,
                ))
            })
            .into_any_element()
    }

    /// 图标视图（M/L/XL）的单个图块（虚拟化后按需构建）。
    /// 尺寸公式必须与 virtual_grid_geometry 一致：w=tile，h=tile*0.74+3*label_line+8。
    #[allow(clippy::too_many_arguments)]
    fn entry_grid_cell(
        &self,
        side: &'static str,
        index: usize,
        entry: &Entry,
        visible_paths: Vec<PathBuf>,
        view_mode: ViewMode,
        tile: f32,
        font_size: f32,
        load_thumbnails: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = entry.path.clone();
        let menu_path = entry.path.clone();
        let is_dir = entry.is_dir;
        let label = entry.name.clone();
        let selected = self.is_selected(side, &path);
        let focused = self.is_focused_selection(side, &path);
        let drag_sources: Vec<PathBuf> = if selected {
            if side == "left" {
                self.left_selected.iter().cloned().collect()
            } else {
                self.right_selected.iter().cloned().collect()
            }
        } else {
            vec![path.clone()]
        };
        let drag_label = if drag_sources.len() > 1 {
            format!("{} 项", drag_sources.len())
        } else {
            label.clone()
        };
        let drop_target = path.clone();
        let label_line_height = ((font_size * 0.86).clamp(12., 18.) * 1.25).ceil();
        let label_lines = 3usize;
        let icon_size = match view_mode {
            ViewMode::MIcons => tile * 0.64,
            ViewMode::LIcons => tile * 0.74,
            ViewMode::XLIcons => tile * 0.82,
            _ => tile * 0.64,
        };
        let image_size = match view_mode {
            ViewMode::MIcons => tile * 0.62,
            ViewMode::LIcons => tile * 0.72,
            ViewMode::XLIcons => tile * 0.78,
            _ => tile * 0.62,
        };
        let visual = if load_thumbnails && !is_dir && is_image_file(&path) {
            let fallback_size = image_size;
            // 零磁盘 IO：元数据来自 Entry（加载时一次 stat）
            if let Some(thumbnail) =
                cached_thumbnail_for_entry(&path, entry.size, entry.modified_millis)
            {
                img(thumbnail)
                    .w(px(image_size))
                    .h(px(image_size))
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(move || file_icon(fallback_size))
                    .into_any_element()
            } else {
                file_icon(fallback_size)
            }
        } else if is_dir {
            folder_icon(icon_size)
        } else if let Some(type_icon) = cached_filetype_icon_path(&path, icon_size) {
            // 系统文件关联图标（zip/exe/pdf 等真实图标），按扩展名缓存
            let fallback_size = icon_size * 0.82;
            img(type_icon)
                .w(px(icon_size * 0.82))
                .h(px(icon_size * 0.82))
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || file_icon(fallback_size))
                .into_any_element()
        } else {
            file_icon(icon_size * 0.82)
        };
        div()
            .id((
                if side == "left" {
                    "entry-grid-left"
                } else {
                    "entry-grid-right"
                },
                index,
            ))
            .w(px(tile))
            // 与 virtual_grid_geometry 的 cell_h 公式严格一致
            .h(px(tile * 0.74 + label_lines as f32 * label_line_height + 8.))
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .rounded_md()
            .bg(if selected {
                rgb(if focused { BLUE } else { HOVER_BLUE })
            } else {
                rgb(0xffffff)
            })
            .hover(move |style| style.bg(rgb(if selected { BLUE } else { HOVER_BLUE })))
            .cursor_move()
            .on_drag(
                FileDragPayload {
                    sources: drag_sources.clone(),
                },
                move |_payload: &FileDragPayload, position, _, cx| {
                    cx.new(|_| FileDragPreview {
                        label: drag_label.clone(),
                        position,
                    })
                },
            )
            .when(is_dir, |item| {
                item.on_drop(cx.listener(
                    move |this, payload: &FileDragPayload, _, cx| {
                        this.drop_files_to(&payload.sources, &drop_target, false, cx);
                    },
                ))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let drag_path = path.clone();
                    let drag_visible_paths = visible_paths.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if event.modifiers.control || event.modifiers.shift {
                            this.select_entry(
                                side,
                                drag_path.clone(),
                                &drag_visible_paths,
                                event.modifiers,
                                cx,
                            );
                            this.file_drag = None;
                        } else {
                            this.begin_file_drag(
                                side,
                                drag_path.clone(),
                                &drag_visible_paths,
                                event.position,
                                event.modifiers,
                                cx,
                            );
                        }
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                cx.stop_propagation();
                this.update_file_drag(event, window, cx);
                this.update_drag_select(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    cx.stop_propagation();
                    if !this.finish_file_drag(event, window, cx) {
                        this.finish_drag_select(event, window, cx);
                    }
                }),
            )
            .child(
                div()
                    // 图标区高度随内容自适应（不同视图图标大小不同）
                    .h(px(icon_size.max(image_size)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(visual),
            )
            .child(
                div()
                    .w(px(tile - 10.))
                    // 实际占高随显示行数自适应，上限 3 行
                    .max_h(px(label_lines as f32 * label_line_height + 4.))
                    .overflow_hidden()
                    .text_center()
                    .line_height(px(label_line_height))
                    .text_size(px((font_size * 0.86).clamp(12., 18.)))
                    .text_color(if selected && focused {
                        rgb(0xffffff)
                    } else {
                        rgb(TEXT)
                    })
                    .line_clamp(label_lines)
                    .child(label),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if this.suppress_blank_click {
                    this.suppress_blank_click = false;
                    cx.notify();
                    return;
                }
                if event.is_right_click() {
                    return;
                }
                if event.modifiers().control || event.modifiers().shift {
                    return;
                }
                this.dismiss_popups_except_new_folder();
                if event.click_count() >= 2 {
                    this.set_single_selection(side, path.clone());
                    this.open_selected(cx);
                } else {
                    this.select_entry(
                        side,
                        path.clone(),
                        &visible_paths,
                        event.modifiers(),
                        cx,
                    );
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.cancel_drag_select();
                    this.begin_context_target(
                        side,
                        menu_path.clone(),
                        event.position,
                        cx,
                    );
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    // 真实路径的合并 Shell 菜单已在 mouse_down 弹出，这里只补虚拟路径 pending
                    this.finish_context_target(event.position, cx);
                }),
            )
            .into_any_element()
    }

    /// 分栏视图的单个条目（虚拟化后按需构建）。w=240，h 由调用方锚定。
    #[allow(clippy::too_many_arguments)]
    fn entry_column_cell(
        &self,
        side: &'static str,
        index: usize,
        entry: &Entry,
        visible_paths: Vec<PathBuf>,
        _view_mode: ViewMode,
        item_w: f32,
        cell_h: f32,
        font_size: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = entry.path.clone();
        let menu_path = entry.path.clone();
        let is_dir = entry.is_dir;
        let name = entry.name.clone();
        let selected = self.is_selected(side, &path);
        let focused = self.is_focused_selection(side, &path);
        let drag_sources: Vec<PathBuf> = if selected {
            if side == "left" {
                self.left_selected.iter().cloned().collect()
            } else {
                self.right_selected.iter().cloned().collect()
            }
        } else {
            vec![path.clone()]
        };
        let drag_label = if drag_sources.len() > 1 {
            format!("{} 项", drag_sources.len())
        } else {
            name.clone()
        };
        let drop_target = path.clone();
        div()
            .id((
                if side == "left" {
                    "entry-column-left"
                } else {
                    "entry-column-right"
                },
                index,
            ))
            .w(px(item_w))
            .h(px(cell_h))
            .flex_none()
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded_sm()
            .bg(if selected {
                rgb(if focused { BLUE } else { HOVER_BLUE })
            } else {
                rgb(0xffffff)
            })
            .hover(move |style| style.bg(rgb(if selected { BLUE } else { HOVER_BLUE })))
            .cursor_move()
            .on_drag(
                FileDragPayload {
                    sources: drag_sources.clone(),
                },
                move |_payload: &FileDragPayload, position, _, cx| {
                    cx.new(|_| FileDragPreview {
                        label: drag_label.clone(),
                        position,
                    })
                },
            )
            .when(is_dir, |item| {
                item.on_drop(cx.listener(
                    move |this, payload: &FileDragPayload, _, cx| {
                        this.drop_files_to(&payload.sources, &drop_target, false, cx);
                    },
                ))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let drag_path = path.clone();
                    let drag_visible_paths = visible_paths.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if event.modifiers.control || event.modifiers.shift {
                            this.select_entry(
                                side,
                                drag_path.clone(),
                                &drag_visible_paths,
                                event.modifiers,
                                cx,
                            );
                            this.file_drag = None;
                        } else {
                            this.begin_file_drag(
                                side,
                                drag_path.clone(),
                                &drag_visible_paths,
                                event.position,
                                event.modifiers,
                                cx,
                            );
                        }
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                cx.stop_propagation();
                this.update_file_drag(event, window, cx);
                this.update_drag_select(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    cx.stop_propagation();
                    if !this.finish_file_drag(event, window, cx) {
                        this.finish_drag_select(event, window, cx);
                    }
                }),
            )
            .child(if is_dir {
                folder_icon(17.)
            } else {
                inline_file_icon(&path, 15.)
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_size(px(font_size))
                    .text_color(if selected && focused {
                        rgb(0xffffff)
                    } else {
                        rgb(TEXT)
                    })
                    .truncate()
                    .child(name),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if this.suppress_blank_click {
                    this.suppress_blank_click = false;
                    cx.notify();
                    return;
                }
                if event.is_right_click() {
                    return;
                }
                if event.modifiers().control || event.modifiers().shift {
                    return;
                }
                this.dismiss_popups_except_new_folder();
                if event.click_count() >= 2 {
                    this.set_single_selection(side, path.clone());
                    this.open_selected(cx);
                } else {
                    this.select_entry(
                        side,
                        path.clone(),
                        &visible_paths,
                        event.modifiers(),
                        cx,
                    );
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.cancel_drag_select();
                    this.begin_context_target(
                        side,
                        menu_path.clone(),
                        event.position,
                        cx,
                    );
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    // 真实路径的合并 Shell 菜单已在 mouse_down 弹出，这里只补虚拟路径 pending
                    this.finish_context_target(event.position, cx);
                }),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn blank_details_cell(
        &self,
        side: &'static str,
        id: usize,
        width: f32,
        right_offset: f32,
        text: String,
        selected: bool,
        focused: bool,
        #[allow(unused_variables)] align_right: bool,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id((
                if side == "left" {
                    "blank-cell-left"
                } else {
                    "blank-cell-right"
                },
                id,
            ))
            .absolute()
            .right(px(right_offset))
            .top(px(0.))
            .bottom(px(0.))
            .w(px(width))
            .flex()
            .items_center()
            .overflow_hidden()
            .truncate()
            .text_color(if selected && focused {
                rgb(0xffffff)
            } else {
                rgb(MUTED)
            })
            .text_sm()
            // 三列统一左对齐。text_right() 在本 flex 容器上对 .child(text) 无效，
            // 必须用 justify_start()（flex 主轴对齐）才能真正左对齐。
            .justify_start()
            .child(text)
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if !event.is_right_click() {
                    this.on_blank_area_click(side, event, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.cancel_drag_select();
                    this.begin_context_target(side, path.clone(), event.position, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    // 真实路径的合并 Shell 菜单已在 mouse_down 弹出，这里只补虚拟路径 pending
                    this.finish_context_target(event.position, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.on_blank_area_mouse_down(side, event, cx);
                }),
            )
    }

    fn status_bar(&self, side: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = if side == "left" {
            &self.left_entries
        } else {
            &self.right_entries
        };
        let view_mode = self.view_mode_for(side);
        let folders = entries.iter().filter(|entry| entry.is_dir).count();
        let files = entries.len().saturating_sub(folders);
        // 选中分类计数：选中时各自的计数变成 选中/总数，并高亮
        let selected = if side == "left" {
            &self.left_selected
        } else {
            &self.right_selected
        };
        let selected_in_view: Vec<&Entry> = entries
            .iter()
            .filter(|entry| selected.contains(&entry.path))
            .collect();
        let selected_folders = selected_in_view
            .iter()
            .filter(|entry| entry.is_dir)
            .count();
        let selected_files = selected_in_view.len().saturating_sub(selected_folders);
        div()
            .h(px(40.))
            .flex_none()
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_t_1()
            .border_color(rgb(0xd9dfe3))
            .bg(rgb(0xfafcfd))
            .child(
                div()
                    .id(if side == "left" {
                        "filter-input-frame-left"
                    } else {
                        "filter-input-frame-right"
                    })
                    .w(px(210.))
                    .h(px(29.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xd9dfe3))
                    .text_color(rgb(MUTED))
                    .child("⌕")
                    .child(
                        div()
                            .pl_2()
                            .flex_1()
                            .h_full()
                            .child(if side == "left" {
                                self.filter_input.clone()
                            } else {
                                self.right_filter_input.clone()
                            }),
                    ),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(rgb(0xd9dfe3)))
            .child(
                div()
                    .id(if side == "left" {
                        "sort-cycle-left"
                    } else {
                        "sort-cycle-right"
                    })
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_1()
                    .hover(|style| style.bg(rgb(HOVER_BLUE)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.active_side = side;
                            if event.modifiers.control {
                                // Ctrl+点击：原地切换升序/降序
                                let (column, ascending) = this.sort_state_for(side);
                                this.sort_by(side, column, cx);
                                // sort_by 同列会翻转；但若刚才是别的列被 Ctrl 点到，
                                // 保持显式：直接设为 !ascending
                                let (_, now_asc) = this.sort_state_for(side);
                                if now_asc == ascending {
                                    this.set_sort_ascending(side, !ascending, cx);
                                }
                                this.status = format!(
                                    "{}栏排序：{} {}",
                                    if side == "left" { "左" } else { "右" },
                                    sort_column_label(column),
                                    if !ascending { "升序" } else { "降序" },
                                );
                            } else {
                                // 普通点击：循环切换排序类型
                                this.cycle_sort(cx);
                            }
                        }),
                    )
                    .child(sort_column_label(self.sort_state_for(side).0))
                    .child(
                        div()
                            .text_size(px(10.))
                            .child(if self.sort_state_for(side).1 {
                                "↑"
                            } else {
                                "↓"
                            }),
                    ),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(rgb(0xd9dfe3)))
            .child(
                div()
                    .id(if side == "left" {
                        "folders-left"
                    } else {
                        "folders-right"
                    })
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_2()
                    .hover(|style| style.bg(rgb(HOVER_BLUE)))
                    // 选中文件夹时蓝色高亮 + 选中/总数 形式
                    .when(selected_folders > 0, |chip| {
                        chip.bg(rgb(HOVER_BLUE))
                            .text_color(rgb(0x185FA5))
                    })
                    .child(folder_icon(16.))
                    .child(if selected_folders > 0 {
                        format!("{selected_folders}/{folders}")
                    } else {
                        folders.to_string()
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_folders(cx))),
            )
            .child(
                div()
                    .id(if side == "left" {
                        "files-left"
                    } else {
                        "files-right"
                    })
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_1()
                    // 选中文件时蓝色高亮 + 选中/总数 形式
                    .when(selected_files > 0, |chip| {
                        chip.bg(rgb(HOVER_BLUE))
                            .text_color(rgb(0x185FA5))
                    })
                    .child("▱")
                    .child(if selected_files > 0 {
                        format!("{selected_files}/{files}")
                    } else {
                        files.to_string()
                    }),
            )
            .child(div().w(px(1.)).h(px(22.)).bg(rgb(0xd9dfe3)))
            .child(div().flex_1())
            .child(
                div()
                    .id("view-down")
                    .px_2()
                    .py_1()
                    .hover(|style| style.bg(rgb(HOVER_BLUE)))
                    .child(format!("▧  {}", view_mode.label()))
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_view(1, cx))),
            )
            .child(
                div()
                    .id("zoom")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(BLUE))
                    .text_color(rgb(0xffffff))
                    .child(view_percent(view_mode).to_string()),
            )
    }

    fn new_folder_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("new-item-overlay")
            .absolute()
            .top(px(96.))
            .left(px(0.))
            .right(px(0.))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("new-item-dialog")
                    .w(px(520.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .rounded_lg()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xb7c1c8))
                    // 独立遮挡命中区域：保留子控件事件，阻断下面的文件列表。
                    .occlude()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_up(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_up(MouseButton::Right, cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_mouse_move(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .child(
                        div()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(rgb(TEXT))
                            .child(div().text_size(px(18.)).child(format!("新建{}", self.new_item_kind.label())))
                            .child(
                                div()
                                    .id("new-folder-close")
                                    .px_2()
                                    .text_color(rgb(MUTED))
                                    .child("×")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.new_folder_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(div().text_sm().text_color(rgb(MUTED)).child("每行一个名称；Shift+Enter / Ctrl+N 换行，Enter 创建"))
                    .child(
                        div()
                            .h(px(144.))
                            .px_2()
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(rgb(BLUE))
                            .rounded_sm()
                            .child(self.new_folder_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("new-folder-cancel")
                                    .px_4()
                                    .py_1()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xc8d1d7))
                                    .child("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.new_folder_open = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("new-folder-create")
                                    .px_4()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(BLUE))
                                    .text_color(rgb(0xffffff))
                                    .child("创建")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.on_submit_address(&SubmitAddress, window, cx)
                                    })),
                            ),
                    ),
            )
    }

    fn rename_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("rename-overlay")
            .absolute()
            .top(px(110.))
            .left(px(0.))
            .right(px(0.))
            .flex()
            .justify_center()
            // 弹框内部点击不冒泡到下层条目/空白区（否则触发 dismiss 把弹框关掉）
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(460.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .rounded_lg()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xb7c1c8))
                    .child(div().flex().justify_between().child("重命名").child(
                        div().id("rename-close").child("×").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.rename_open = false;
                                this.rename_target = None;
                                cx.notify();
                            },
                        )),
                    ))
                    .child(
                        div()
                            .h(px(34.))
                            .px_2()
                            .border_1()
                            .border_color(rgb(BLUE))
                            .rounded_sm()
                            .child(self.rename_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("rename-cancel")
                                    .px_3()
                                    .py_1()
                                    .border_1()
                                    .border_color(rgb(0xc8d1d7))
                                    .child("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.rename_open = false;
                                        this.rename_target = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("rename-apply")
                                    .px_3()
                                    .py_1()
                                    .bg(rgb(BLUE))
                                    .text_color(rgb(0xffffff))
                                    .child("确定")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.on_submit_address(&SubmitAddress, window, cx)
                                    })),
                            ),
                    ),
            )
    }

    /// 批量重命名对话框：名称 + 序号（名称_001、名称_002…），带预览
    fn batch_rename_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.batch_rename_targets.len();
        let base = self.batch_rename_input.read(cx).value().trim().to_string();
        let base_display: &str = if base.is_empty() { "名称" } else { &base };
        let width = count.to_string().len().max(3);
        // 预览：前 2 项 + 最后一项（不足则全部）
        let preview: Vec<String> = (0..count)
            .filter(|index| *index < 2 || *index == count - 1)
            .map(|index| {
                let target = &self.batch_rename_targets[index];
                let extension = target
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| format!(".{value}"))
                    .unwrap_or_default();
                let number = self.batch_rename_start + index as u32;
                let old = target
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let new = format!("{base_display}_{number:0width$}{extension}", width = width);
                if count > 3 && index == 2 {
                    format!("{old}  →  …")
                } else {
                    format!("{old}  →  {new}")
                }
            })
            .collect();
        div()
            .id("batch-rename-overlay")
            .absolute()
            .top(px(110.))
            .left(px(0.))
            .right(px(0.))
            .flex()
            .justify_center()
            // 弹框内部点击不冒泡到下层（否则触发 dismiss 把弹框关掉）
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(460.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .rounded_lg()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xb7c1c8))
                    // 拦截鼠标：点击不穿透到下方文件区（否则点弹框任意处会被
                    // 空白区处理器当作点击空白关闭弹框）
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(format!("批量重命名（{count} 项）"))
                            .child(div().id("batch-rename-close").child("×").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.batch_rename_open = false;
                                    this.batch_rename_targets.clear();
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(
                        div()
                            .h(px(34.))
                            .px_2()
                            .border_1()
                            .border_color(rgb(BLUE))
                            .rounded_sm()
                            .child(self.batch_rename_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(rgb(MUTED))
                            .child("起始序号")
                            .child(
                                div()
                                    .id("batch-rename-dec")
                                    .px_2()
                                    .border_1()
                                    .border_color(rgb(0xc8d1d7))
                                    .rounded_sm()
                                    .child("−")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.batch_rename_start > 0 {
                                            this.batch_rename_start -= 1;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .w(px(40.))
                                    .text_center()
                                    .child(self.batch_rename_start.to_string()),
                            )
                            .child(
                                div()
                                    .id("batch-rename-inc")
                                    .px_2()
                                    .border_1()
                                    .border_color(rgb(0xc8d1d7))
                                    .rounded_sm()
                                    .child("＋")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.batch_rename_start += 1;
                                        cx.notify();
                                    })),
                            )
                            .child(format!(
                                "（生成 {}_{:0width$} 起）",
                                base_display,
                                self.batch_rename_start,
                                width = width
                            )),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_sm()
                            .bg(rgb(0xf4f7f9))
                            .text_color(rgb(MUTED))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(
                                preview
                                    .into_iter()
                                    .map(|line| div().text_size(px(12.)).truncate().child(line)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("batch-rename-cancel")
                                    .px_3()
                                    .py_1()
                                    .border_1()
                                    .border_color(rgb(0xc8d1d7))
                                    .child("取消")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.batch_rename_open = false;
                                        this.batch_rename_targets.clear();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("batch-rename-apply")
                                    .px_3()
                                    .py_1()
                                    .bg(rgb(BLUE))
                                    .text_color(rgb(0xffffff))
                                    .child(format!("重命名 {count} 项"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.on_submit_address(&SubmitAddress, window, cx)
                                    })),
                            ),
                    ),
            )
    }

    fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let option = |id: &'static str, label: &'static str, value: String| {
            div()
                .id(id)
                .h(px(36.))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(rgb(0xe8edef))
                .child(label)
                .child(value)
        };
        div()
            .id("settings-panel")
            .absolute()
            .top(px(42.))
            .right(px(8.))
            .w(px(430.))
            .max_h(px(790.))
            .overflow_y_scroll()
            .p_3()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xb7c1c8))
            // 拦截鼠标：面板覆盖的文件不再 hover 半选、拖选穿透
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_2()
                    .child("选项")
                    .child(div().id("settings-close").child("×").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.show_settings = false;
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .pb_2()
                    .child("所有修改立即生效并保存"),
            )
            .child(option(
                "settings-font",
                "界面字体",
                "Sarasa UI SC".to_string(),
            ))
            .child(
                div()
                    .id("settings-ui-font")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("界面字体大小")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().id("ui-font-minus").px_2().child("−").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.ui_font_size = (this.ui_font_size - 1.).max(11.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            ))
                            .child(format!("{:.0}", self.ui_font_size))
                            .child(div().id("ui-font-plus").px_2().child("＋").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.ui_font_size = (this.ui_font_size + 1.).min(24.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .id("settings-file-font")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("文件字体大小")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().id("file-font-minus").px_2().child("−").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.file_font_size = (this.file_font_size - 1.).max(11.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            ))
                            .child(format!("{:.0}", self.file_font_size))
                            .child(div().id("file-font-plus").px_2().child("＋").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.file_font_size = (this.file_font_size + 1.).min(26.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .id("settings-row-spacing")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("文件上下间距")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().id("row-spacing-minus").px_2().child("−").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.row_spacing = (this.row_spacing - 1.).max(0.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            ))
                            .child(format!("{:.0}", self.row_spacing))
                            .child(div().id("row-spacing-plus").px_2().child("＋").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.row_spacing = (this.row_spacing + 1.).min(24.);
                                    this.save_config();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .id("settings-recent-limit")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("最近文件数量")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().id("recent-limit-minus").px_2().child("−").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.recent_limit = this.recent_limit.saturating_sub(1);
                                    this.recents.truncate(this.recent_limit);
                                    this.save_config();
                                    cx.notify();
                                }),
                            ))
                            .child(self.recent_limit.to_string())
                            .child(div().id("recent-limit-plus").px_2().child("＋").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.recent_limit = (this.recent_limit + 1).min(30);
                                    this.recents.truncate(this.recent_limit);
                                    this.save_config();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .id("settings-thumbnails")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("加载缩略图（含 WebP）")
                    .child(if self.load_thumbnails { "开" } else { "关" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.load_thumbnails = !this.load_thumbnails;
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-hidden")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("显示隐藏文件")
                    .child(if self.show_hidden { "开" } else { "关" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_hidden = !this.show_hidden;
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-win-e")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("接管 Win+E")
                    .child(if self.win_e_enabled { "开" } else { "关" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.win_e_enabled = !this.win_e_enabled;
                        WIN_E_ENABLED.store(this.win_e_enabled, AtomicOrdering::Relaxed);
                        set_win_e_startup(this.win_e_enabled);
                        if this.win_e_enabled {
                            start_win_e_listener();
                        }
                        this.status = if this.win_e_enabled {
                            "Win+E 已开启；FileFlow 会随登录启动并拦截 Win+E"
                        } else {
                            "Win+E 已关闭，并已移除启动项"
                        }
                        .to_string();
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-explorer-replacement")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child("替换资源管理器"),
                    )
                    .child(if self.explorer_replacement { "开" } else { "关" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.explorer_replacement = !this.explorer_replacement;
                        match set_explorer_replacement(this.explorer_replacement) {
                            Ok(()) => {
                                this.status = if this.explorer_replacement {
                                    "已接管文件夹默认打开方式；导出软件\"打开位置\"将调起 FileFlow"
                                } else {
                                    "已恢复系统默认资源管理器"
                                }
                                .to_string();
                            }
                            Err(error) => {
                                this.explorer_replacement = !this.explorer_replacement;
                                this.status = format!("设置失败：{error}");
                            }
                        }
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-shell-menu-default")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("右键菜单管理：默认 Windows 菜单")
                    .child(if self.shell_menu_default {
                        "开"
                    } else {
                        "关"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell_menu_default = !this.shell_menu_default;
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-shell-menu-third-party")
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .child("右键菜单管理：第三方扩展")
                    .child(if self.shell_menu_third_party {
                        "开"
                    } else {
                        "关"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell_menu_third_party = !this.shell_menu_third_party;
                        this.status = if this.shell_menu_third_party {
                            "第三方右键扩展已允许；后续接入系统扩展时会按白名单加载"
                        } else {
                            "第三方右键扩展已关闭，右键保持轻量"
                        }
                        .to_string();
                        this.save_config();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("settings-shortcuts")
                    .mt_2()
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .bg(rgb(HOVER_BLUE))
                    .child("快捷键设置…")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.show_settings = false;
                        this.show_shortcuts = true;
                        cx.notify();
                    })),
            )
    }

    fn shortcuts_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("shortcuts-panel")
            .absolute()
            .top(px(42.))
            .right(px(8.))
            .w(px(430.))
            .p_3()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xb7c1c8))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_2()
                    .child("快捷键")
                    .child(div().id("shortcuts-close").child("×").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.show_shortcuts = false;
                            cx.notify();
                        },
                    ))),
            )
            .children(shortcut_specs().iter().map(|spec| {
                let id = spec.id.to_string();
                let recording = self.shortcut_recording.as_deref() == Some(spec.id);
                let key = self
                    .shortcut_bindings
                    .get(spec.id)
                    .map(String::as_str)
                    .unwrap_or(spec.default_key);
                div()
                    .id(spec.id)
                    .h(px(34.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe8edef))
                    .hover(|style| style.bg(rgb(HOVER_BLUE)))
                    .child(spec.label)
                    .child(
                        div()
                            .px_2()
                            .rounded_sm()
                            .bg(rgb(if recording { BLUE } else { 0xf0f2f4 }))
                            .text_color(rgb(if recording { 0xffffff } else { MUTED }))
                            .child(if recording {
                                "请按新的组合键…".to_string()
                            } else {
                                display_shortcut(key)
                            }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.shortcut_recording = Some(id.clone());
                        this.status = "请按新的组合键；按 Esc 取消".to_string();
                        cx.notify();
                    }))
            }))
            .child(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("点击一项后按键即可立即生效；冲突按键不会覆盖。"),
            )
    }

    fn preview_panel(&self, side: &'static str) -> impl IntoElement {
        let selected = if side == "right" {
            self.right_focused
                .as_ref()
                .filter(|path| self.right_selected.contains(*path))
                .or_else(|| self.right_selected.iter().next())
        } else {
            self.left_focused
                .as_ref()
                .filter(|path| self.left_selected.contains(*path))
                .or_else(|| self.left_selected.iter().next())
        };
        // 渲染零 IO：is_dir/size 取 Entry（加载时已 stat 过），不在帧里 fs::metadata
        let selected_is_dir = selected.as_ref().map(|path| {
            self.entry_of(path)
                .map(|entry| entry.is_dir)
                .unwrap_or_else(|| {
                    is_virtual_path(path) || cached_path_is_dir(path, path.extension().is_none())
                })
        });
        let body: AnyElement = match selected {
            Some(_) if selected_is_dir == Some(true) => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("文件夹预览")
                .into_any_element(),
            Some(path) if is_image_file(path) => img(path.clone())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .with_fallback(|| {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(MUTED))
                        .child("图片预览失败")
                        .into_any_element()
                })
                .into_any_element(),
            Some(path) => div()
                .flex_1()
                .p_4()
                .text_color(rgb(MUTED))
                .child(format!(
                    "{}\n{}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("文件"),
                    format_size(self.entry_of(path).map(|entry| entry.size).unwrap_or(0))
                ))
                .into_any_element(),
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("选择文件后按 Space 预览")
                .into_any_element(),
        };
        div()
            .id(if side == "right" {
                "preview-panel-right"
            } else {
                "preview-panel-left"
            })
            .flex_1()
            .min_w(px(0.))
            .min_h(px(180.))
            .flex()
            .flex_col()
            .border_color(rgb(0xd9dfe3))
            .bg(rgb(0xfbfcfd))
            .when(!self.split, |panel| panel.w(px(0.)))
            .when(self.split, |panel| panel.border_t_1())
            .when(!self.split, |panel| panel.border_l_1())
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xd9dfe3))
                    .child("预览")
                    .child("Space 关闭"),
            )
            .child(body)
    }

    fn type_filter_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = if self.type_filter_side == "right" {
            &self.right_entries
        } else {
            &self.left_entries
        };
        let counts = type_counts(entries);
        let total_types = counts.len();
        let left = if self.type_filter_side == "right" {
            940.
        } else {
            520.
        };
        div()
            .id("type-filter-panel")
            .absolute()
            .top(px(118.))
            .left(px(left))
            .w(px(330.))
            .p_2()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xb7c1c8))
            .shadow_lg()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0xe1e6e9))
                    .child("⌕")
                    .child(
                        div()
                            .flex_1()
                            .text_color(rgb(MUTED))
                            .child(format!("过滤 {total_types} types...")),
                    )
                    .child(
                        div()
                            .id("type-filter-all")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .hover(|style| style.bg(rgb(HOVER_BLUE)))
                            .child("全选")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.type_filter_allowed.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("type-filter-close")
                            .px_2()
                            .text_color(rgb(MUTED))
                            .child("×")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.type_filter_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .children(
                counts
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, count))| {
                        let enabled = self.type_filter_allowed.is_empty()
                            || self.type_filter_allowed.contains(&label);
                        div()
                            .id(("type-filter-row", index))
                            .h(px(34.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded_sm()
                            .bg(if enabled {
                                rgb(HOVER_BLUE)
                            } else {
                                rgb(0xffffff)
                            })
                            .hover(|style| style.bg(rgb(HOVER_BLUE)))
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .text_color(rgb(if enabled { TEXT } else { MUTED }))
                                    .child(label.clone()),
                            )
                            .child(div().text_color(rgb(MUTED)).child(count.to_string()))
                            .child(
                                div()
                                    .w(px(18.))
                                    .h(px(18.))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0x9aa6ad))
                                    .bg(rgb(if enabled { BLUE } else { 0xffffff }))
                                    .child(if enabled { "✓" } else { "" }),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_type_filter(label.clone(), cx)
                            }))
                    }),
            )
    }

    fn address_suggestions_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let suggestions = self.address_suggestions.clone();
        div()
            .absolute()
            .top(px(84.))
            .left(px(322.))
            .w(px(520.))
            .flex()
            .flex_col()
            .rounded_md()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xb7c1c8))
            .shadow_lg()
            // 拦截鼠标穿透到下层文件列表（hover 会误选中下方文件）
            .occlude()
            .children(
                suggestions
                    .into_iter()
                    .enumerate()
                    .map(|(index, suggestion)| {
                        let label = suggestion.display().to_string();
                        div()
                            .id(("address-root-suggestion", index))
                            .w_full()
                            .h(px(32.))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_color(rgb(TEXT))
                            .hover(|style| style.bg(rgb(HOVER_BLUE)))
                            .child(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.address_side == "right" && this.split {
                                    this.navigate_right(suggestion.clone(), cx);
                                } else {
                                    this.navigate_left(suggestion.clone(), cx);
                                }
                                this.address_editing = false;
                                window.focus(&this.focus_handle);
                            }))
                    }),
            )
    }

    fn context_menu_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = if self.context_menu_blank {
            None
        } else {
            self.selected_path()
        };
        let has_selection = selected.is_some();
        let current_folder = if self.context_menu_side == "right" {
            self.right_path.clone()
        } else {
            self.current_path().to_path_buf()
        };
        let favorite_target = selected
            .as_ref()
            .filter(|path| can_open_directory(path))
            .cloned()
            .unwrap_or_else(|| current_folder.clone());
        let is_dir = selected.as_ref().is_some_and(|path| {
            is_virtual_path(path)
                || self
                    .entry_of(path)
                    .map(|entry| entry.is_dir)
                    .unwrap_or_else(|| cached_path_is_dir(path, path.extension().is_none()))
        });
        let is_favorite = self.favorites.iter().any(|item| item == &favorite_target);
        let favorite_label = if is_favorite {
            "取消收藏"
        } else {
            "收藏文件夹"
        };
        let row = |id: &'static str, label: String, enabled: bool| {
            div()
                .id(id)
                .h(px(32.))
                .px_3()
                .flex()
                .items_center()
                .rounded_sm()
                .text_color(rgb(if enabled { TEXT } else { MUTED }))
                .hover(move |style| {
                    if enabled {
                        style.bg(rgb(HOVER_BLUE))
                    } else {
                        style
                    }
                })
                .child(label)
        };
        let position = self.context_menu_position.unwrap_or_else(|| Point {
            x: px(if self.context_menu_side == "left" {
                360.
            } else {
                980.
            }),
            y: px(168.),
        });
        div()
            .id("context-menu-panel")
            .absolute()
            .top(position.y)
            .left(position.x)
            .w(px(300.))
            .p_1()
            .flex()
            .flex_col()
            .rounded_md()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xb7c1c8))
            .shadow_lg()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .when(self.context_menu_blank, |menu| {
                // 空白处 GPUI 菜单仅用于虚拟路径；真实路径已走 Shell 合并菜单
                menu.child(
                    row("ctx-new-folder", "新建文件夹(N)".to_string(), true).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.context_menu_open = false;
                            this.on_new_folder(&NewFolder, window, cx);
                        },
                    )),
                )
                .child(
                    row("ctx-new-text", "新建 TXT 文档".to_string(), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.create_text_file(cx);
                        },
                    )),
                )
                .child(
                    row("ctx-view", "查看(V)".to_string(), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.cycle_view(1, cx);
                        },
                    )),
                )
                .child(
                    row("ctx-sort-name", "按名称排序".to_string(), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.sort_by(this.context_menu_side, SortColumn::Name, cx);
                        },
                    )),
                )
                .child(
                    row("ctx-refresh", "刷新(E)".to_string(), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.refresh(cx);
                        },
                    )),
                )
                .child(div().h(px(1.)).mx_1().my_1().bg(rgb(0xe1e6e9)))
                .child(
                    row("ctx-open-cmd", "在此处打开命令提示符".to_string(), true).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.context_menu_open = false;
                            this.open_command_prompt_here(cx);
                        }),
                    ),
                )
                .child(
                    row("ctx-paste", "粘贴(P)  Ctrl+V".to_string(), true).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.context_menu_open = false;
                            this.on_paste_files(&PasteFiles, window, cx);
                        },
                    )),
                )
                .child(div().h(px(1.)).mx_1().my_1().bg(rgb(0xe1e6e9)))
                .child(
                    row("ctx-folder-properties", "属性(R)".to_string(), true).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.context_menu_open = false;
                            let folder = current_folder.clone();
                            this.status = if shell_execute_verb(&folder, "properties") {
                                "已打开 Windows 属性"
                            } else {
                                "打开属性失败"
                            }
                            .to_string();
                            cx.notify();
                        }),
                    ),
                )
            })
            .when(!self.context_menu_blank, |menu| {
                // 虚拟路径（此电脑/网络/回收站）的 GPUI 回退菜单；
                // 真实路径已改用 Shell 合并菜单（open_merged_context_menu_at）
                menu.child(
                    row("ctx-open", "打开".to_string(), has_selection).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.open_selected(cx);
                        },
                    )),
                )
                .child(
                    row("ctx-open-other", "在另一栏打开".to_string(), is_dir).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.context_menu_open = false;
                            this.open_selected_in_other(cx);
                        }),
                    ),
                )
                .child(div().h(px(1.)).mx_1().my_1().bg(rgb(0xe1e6e9)))
                .child(
                    row("ctx-favorite", favorite_label.to_string(), true).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.context_menu_open = false;
                            this.on_toggle_favorite(&ToggleFavorite, window, cx);
                        },
                    )),
                )
                .child(
                    row("ctx-native-menu", "Windows 完整右键菜单".to_string(), has_selection).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.context_menu_open = false;
                            this.open_native_context_menu(cx);
                        }),
                    ),
                )
                .child(
                    row("ctx-explorer", "在资源管理器中打开".to_string(), true).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.context_menu_open = false;
                            this.open_in_explorer(cx);
                        }),
                    ),
                )
                .child(
                    row("ctx-reveal", "在资源管理器中定位".to_string(), has_selection).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.context_menu_open = false;
                            this.reveal_in_explorer(cx);
                        }),
                    ),
                )
                .child(
                    row("ctx-refresh", "刷新  F5".to_string(), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.context_menu_open = false;
                            this.refresh(cx);
                        },
                    )),
                )
            })
    }

    fn drag_select_overlay(&self) -> impl IntoElement {
        let (left, top, width, height) =
            if let Some(state) = self.drag_select.as_ref().filter(|state| state.active) {
                let x1 = pixel_value(state.start.x).min(pixel_value(state.current.x));
                let x2 = pixel_value(state.start.x).max(pixel_value(state.current.x));
                let y1 = pixel_value(state.start.y).min(pixel_value(state.current.y));
                let y2 = pixel_value(state.start.y).max(pixel_value(state.current.y));
                (x1, y1, (x2 - x1).max(1.0), (y2 - y1).max(1.0))
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
        let dash = 12.0;
        let gap = 8.0;
        let h_segments = ((width / (dash + gap)).ceil() as usize).max(1);
        let v_segments = ((height / (dash + gap)).ceil() as usize).max(1);
        let top_dashes = (0..h_segments).map(move |index| {
            let x = (index as f32 * (dash + gap)).min(width);
            div()
                .absolute()
                .left(px(x))
                .top(px(0.))
                .w(px(dash.min(width - x).max(1.)))
                .h(px(2.))
                .bg(rgb(BLUE))
        });
        let bottom_dashes = (0..h_segments).map(move |index| {
            let x = (index as f32 * (dash + gap)).min(width);
            div()
                .absolute()
                .left(px(x))
                .bottom(px(0.))
                .w(px(dash.min(width - x).max(1.)))
                .h(px(2.))
                .bg(rgb(BLUE))
        });
        let left_dashes = (0..v_segments).map(move |index| {
            let y = (index as f32 * (dash + gap)).min(height);
            div()
                .absolute()
                .left(px(0.))
                .top(px(y))
                .w(px(2.))
                .h(px(dash.min(height - y).max(1.)))
                .bg(rgb(BLUE))
        });
        let right_dashes = (0..v_segments).map(move |index| {
            let y = (index as f32 * (dash + gap)).min(height);
            div()
                .absolute()
                .right(px(0.))
                .top(px(y))
                .w(px(2.))
                .h(px(dash.min(height - y).max(1.)))
                .bg(rgb(BLUE))
        });
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .bg(rgba(0xABE3F766))
            .children(top_dashes)
            .children(bottom_dashes)
            .children(left_dashes)
            .children(right_dashes)
    }
}

impl Render for FileFlowGpui {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left_path = self.current_path().to_path_buf();
        let right_path = self.right_path.clone();
        let left_query = self.filter_input.read(cx).value();
        let right_query = self.right_filter_input.read(cx).value();
        let left_entries = self.arranged_entries(&self.left_entries, &left_query, "left");
        let right_entries = self.arranged_entries(&self.right_entries, &right_query, "right");
        // 缓存"渲染视图数据"：uniform_list 闭包异步构建行时必须读到与当前帧
        // 完全一致的过滤/排序结果（不能重读原始 left/right_entries——那会绕过
        // 搜索/隐藏/类型过滤与排序，导致搜索失效、新文件错位、顺序错乱）
        self.left_display_entries = left_entries.clone();
        self.right_display_entries = right_entries.clone();
        // 缓存窗口宽度：Details 列宽自适应需要它（行构建闭包拿不到 window）
        self.last_pane_width = f32::from(window.bounds().size.width).max(280.0);
        div()
            .id("fileflow-root")
            .relative()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_refresh))
            .on_action(cx.listener(Self::on_up))
            .on_action(cx.listener(Self::on_back))
            .on_action(cx.listener(Self::on_forward))
            .on_action(cx.listener(Self::on_split))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_next_view))
            .on_action(cx.listener(Self::on_previous_view))
            .on_action(cx.listener(Self::on_new_tab))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_new_folder))
            .on_action(cx.listener(Self::on_new_text_file))
            .on_action(cx.listener(Self::on_hidden))
            .on_action(cx.listener(Self::on_folder_order))
            .on_action(cx.listener(Self::on_copy_other))
            .on_action(cx.listener(Self::on_move_other))
            .on_action(cx.listener(Self::on_copy_paths))
            .on_action(cx.listener(Self::on_focus_filter))
            .on_action(cx.listener(Self::on_focus_address))
            .on_action(cx.listener(Self::on_clear_transient))
            .on_action(cx.listener(Self::on_submit_address))
            .on_action(cx.listener(Self::on_open_settings))
            .on_action(cx.listener(Self::on_open_shortcuts))
            .on_action(cx.listener(Self::on_toggle_preview))
            .on_action(cx.listener(Self::on_toggle_favorite))
            .on_action(cx.listener(Self::on_select_all_entries))
            .on_action(cx.listener(Self::on_open_properties))
            .on_action(cx.listener(Self::on_copy_selected))
            .on_action(cx.listener(Self::on_cut_selected))
            .on_action(cx.listener(Self::on_paste_files))
            .on_action(cx.listener(Self::on_rename_selected))
            .on_action(cx.listener(Self::on_delete_selected))
            .on_action(cx.listener(Self::on_permanent_delete_selected))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_navigate_up))
            .on_action(cx.listener(Self::on_navigate_down))
            .on_action(cx.listener(Self::on_navigate_left))
            .on_action(cx.listener(Self::on_navigate_right))
            .on_action(cx.listener(Self::on_view_details))
            .on_action(cx.listener(Self::on_view_list))
            .on_action(cx.listener(Self::on_view_columns))
            .on_action(cx.listener(Self::on_view_m_icons))
            .on_action(cx.listener(Self::on_view_l_icons))
            .on_action(cx.listener(Self::on_view_xl_icons))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if event.modifiers.control {
                    if this.split {
                        let x = f32::from(event.position.x);
                        let width = f32::from(window.bounds().size.width);
                        let split_at = 218.0 + ((width - 218.0).max(0.0) / 2.0);
                        this.active_side = if x >= split_at { "right" } else { "left" };
                    }
                    let delta = match event.delta {
                        ScrollDelta::Pixels(delta) => f32::from(delta.y),
                        ScrollDelta::Lines(delta) => delta.y,
                    };
                    if delta != 0.0 {
                        this.cycle_view(if delta > 0.0 { 1 } else { -1 }, cx);
                    }
                }
            }))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, _, cx| {
                    this.go_back(this.active_side, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, _, cx| {
                    this.go_forward(this.active_side, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_file_drag(event, window, cx);
                this.update_drag_select(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if !this.finish_file_drag(event, window, cx) {
                        this.finish_drag_select(event, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, event: &MouseUpEvent, _, cx| {
                    this.finish_context_target(event.position, cx);
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .text_color(rgb(TEXT))
            .text_size(px(self.ui_font_size))
            .when(!self.split, |root| root.child(self.tab_bar(true, cx)))
            .child(
                div().flex_1().min_w(px(0.)).overflow_hidden().child(
                    div()
                        .h_full()
                        .flex()
                        .min_w(px(0.))
                        .when(self.show_sidebar, |layout| layout.child(self.sidebar(cx)))
                        .child(
                            div()
                                .id(("view-transition", self.view_epoch))
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .w(px(0.))
                                        .min_w(px(0.))
                                        .overflow_hidden()
                                        .border_r_1()
                                        .border_color(rgb(0xd9dfe3))
                                        .when(self.split, |pane| {
                                            pane.child(self.tab_bar(false, cx))
                                        })
                                        .child(self.pane_header(&left_path, "left", cx))
                                        .child(self.entries_view(&left_entries, "left", cx))
                                        .child(self.status_bar("left", cx))
                                        .when(self.left_preview_open && self.split, |pane| {
                                            pane.child(self.preview_panel("left"))
                                        }),
                                )
                                .when(self.split, |layout| {
                                    layout.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .w(px(0.))
                                            .min_w(px(0.))
                                            .overflow_hidden()
                                            .child(self.right_tab_bar(true, cx))
                                            .child(self.pane_header(&right_path, "right", cx))
                                            .child(self.entries_view(&right_entries, "right", cx))
                                            .child(self.status_bar("right", cx))
                                            .when(self.right_preview_open, |pane| {
                                                pane.child(self.preview_panel("right"))
                                            }),
                                    )
                                })
                                .when(self.left_preview_open && !self.split, |layout| {
                                    layout.child(self.preview_panel("left"))
                                })
                                .with_animation(
                                    ("view-fade", self.view_epoch),
                                    Animation::new(Duration::from_millis(140)),
                                    |element, delta| element.opacity(0.72 + delta * 0.28),
                                ),
                        ),
                ),
            )
            .when(self.new_folder_open, |root| {
                root.child(self.new_folder_dialog(cx))
            })
            .when(self.rename_open, |root| root.child(self.rename_dialog(cx)))
            .when(self.batch_rename_open, |root| root.child(self.batch_rename_dialog(cx)))
            .when(
                self.address_editing && !self.address_suggestions.is_empty(),
                |root| root.child(self.address_suggestions_panel(cx)),
            )
            .when(self.context_menu_open, |root| {
                root.child(self.context_menu_panel(cx))
            })
            .when(self.type_filter_open, |root| {
                root.child(self.type_filter_panel(cx))
            })
            .when(self.show_settings, |root| {
                root.child(self.settings_panel(cx))
            })
            .when(self.show_shortcuts, |root| {
                root.child(self.shortcuts_panel(cx))
            })
            .when(
                self.drag_select.as_ref().is_some_and(|state| state.active),
                |root| root.child(self.drag_select_overlay()),
            )
    }
}

/// 用户成功访问过的 UNC 共享记忆：NAS 拒绝枚举共享列表时，仍能把这些列出来。
/// key = 服务器名，value = 共享名集合。
static KNOWN_UNC_SHARES: OnceLock<Mutex<std::collections::HashMap<String, std::collections::BTreeSet<String>>>> =
    OnceLock::new();

/// 导航/打开 \\\\server\\share 成功时登记，供服务器根视图回显
fn remember_unc_share(path: &Path) {
    let text = path.as_os_str().to_string_lossy();
    if !text.starts_with("\\\\") {
        return;
    }
    let parts: Vec<&str> = text
        .trim_start_matches('\\')
        .split('\\')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return;
    }
    let server = parts[0].to_string();
    let share = parts[1].to_string();
    let map = KNOWN_UNC_SHARES.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(mut map) = map.lock() {
        map.entry(server).or_default().insert(share);
    }
}

/// 服务器根视图：枚举被拒时，回显用户访问过的共享（有则不再显示"不提供列表"提示）
fn known_unc_share_entries(server: &str) -> Vec<Entry> {
    let Some(map) = KNOWN_UNC_SHARES.get() else {
        return Vec::new();
    };
    let Ok(map) = map.lock() else {
        return Vec::new();
    };
    let Some(shares) = map.get(server) else {
        return Vec::new();
    };
    shares
        .iter()
        .map(|share| Entry {
            name: share.clone(),
            path: PathBuf::from(format!("\\\\{server}\\{share}")),
            is_dir: true,
            size: 0,
            modified: "网络共享".to_string(),
            modified_millis: 0,
        })
        .collect()
}

/// UNC 服务器根的枚举负缓存：NAS 拒绝共享枚举（NetShareEnum/WNet/read_dir 全被拒）
/// 时，read_dir 每次要吃 2.7s SMB 超时。这里记下"枚举被拒"的服务器，30 秒内
/// 不再重试，直接给出提示条目。
static UNC_ENUM_DENIED: OnceLock<Mutex<Vec<(String, Instant)>>> = OnceLock::new();

fn mark_unc_enum_denied(server: &str) {
    let cache = UNC_ENUM_DENIED.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut entries) = cache.lock() {
        entries.retain(|(_, at)| at.elapsed() < Duration::from_secs(30));
        entries.push((server.to_string(), Instant::now()));
    }
}

fn unc_enum_recently_denied(server: &str) -> bool {
    let Some(cache) = UNC_ENUM_DENIED.get() else {
        return false;
    };
    if let Ok(entries) = cache.lock() {
        entries
            .iter()
            .any(|(name, at)| name == server && at.elapsed() < Duration::from_secs(30))
    } else {
        false
    }
}

fn read_entries(path: &Path) -> Vec<Entry> {
    if is_unc_server_root(path) {
        let server = unc_server_name(path).unwrap_or_default();
        let shares = network_share_entries(path);
        if !shares.is_empty() {
            return shares;
        }
        // NAS 拒绝共享枚举 API 时，read_dir 仍可能列出共享（SMB 会话热时
        // 可成功，慢一些）。这里在后台线程耐心等（不卡 UI），失败再回退
        // 已知共享记忆。成功的结果顺带喂给共享记忆。
        if let Ok(iterator) = fs::read_dir(path) {
            let mut entries: Vec<_> = iterator
                .filter_map(Result::ok)
                .map(entry_from_fs_dir_entry)
                .collect();
            if !entries.is_empty() {
                for entry in &entries {
                    if entry.is_dir {
                        remember_unc_share(&entry.path);
                    }
                }
                entries.sort_by(|left, right| {
                    right
                        .is_dir
                        .cmp(&left.is_dir)
                        .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                });
                return entries;
            }
        }
        mark_unc_enum_denied(&server);
        let known = known_unc_share_entries(&server);
        if !known.is_empty() {
            return known;
        }
        return vec![network_denied_entry(&server)];
    }
    let mut entries: Vec<_> = fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(entry_from_fs_dir_entry)
        .collect();
    entries.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    entries
}

/// 从 std::fs::DirEntry 构造 Entry（统一 modified 字符串与毫秒时间戳）
fn entry_from_fs_dir_entry(entry: fs::DirEntry) -> Entry {
    let path = entry.path();
    let metadata = entry.metadata().ok();
    let (modified, modified_millis) = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .map(|time| {
            (
                DateTime::<Local>::from(time)
                    .format("%Y-%m-%d %H:%M")
                    .to_string(),
                // 存 Windows FILETIME（100ns 单位）：与旧版 SystemTime::hash 的
                // 写入值完全一致，缩略图缓存 key 因此向前兼容（老缓存不失效）
                time.duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| {
                        (duration.as_secs() + 11_644_473_600) * 10_000_000
                            + duration.subsec_nanos() as u64 / 100
                    })
                    .unwrap_or(0),
            )
        })
        .unwrap_or_else(|| ("--".to_string(), 0));
    Entry {
        name: entry.file_name().to_string_lossy().to_string(),
        is_dir: metadata.as_ref().is_some_and(|meta| meta.is_dir()),
        size: metadata.map(|meta| meta.len()).unwrap_or(0),
        modified,
        modified_millis,
        path,
    }
}

/// NAS 拒绝枚举共享时的提示条目
fn network_denied_entry(server: &str) -> Entry {
    Entry {
        name: "此服务器不提供共享列表".to_string(),
        path: PathBuf::from(format!("\\\\{server}")),
        is_dir: false,
        size: 0,
        modified: "在地址栏输入 \\\\服务器\\共享名 直达".to_string(),
        modified_millis: 0,
    }
}

fn path_from_user_input(value: &str) -> PathBuf {
    let trimmed = value.trim();
    if trimmed.starts_with("//") && !trimmed.starts_with("///") {
        PathBuf::from(normalize_unc_text(&format!(
            "\\\\{}",
            trimmed.trim_start_matches('/').replace('/', "\\")
        )))
    } else if trimmed.starts_with("\\\\") {
        PathBuf::from(normalize_unc_text(trimmed))
    } else {
        normalize_path(PathBuf::from(trimmed.replace('/', "\\")))
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    if text.starts_with("//") && !text.starts_with("///") {
        PathBuf::from(normalize_unc_text(&format!(
            "\\\\{}",
            text.trim_start_matches('/').replace('/', "\\")
        )))
    } else if text.starts_with("\\\\") {
        PathBuf::from(normalize_unc_text(&text))
    } else if text.as_bytes().get(1) == Some(&b':')
        && text
            .as_bytes()
            .get(2)
            .is_none_or(|separator| *separator != b'\\')
    {
        PathBuf::from(format!("{}\\{}", &text[..2], &text[2..]))
    } else {
        path
    }
}

fn normalize_unc_text(value: &str) -> String {
    let normalized = value.replace('/', "\\");
    let without_prefix = normalized.trim_start_matches('\\');
    let mut parts = without_prefix.split('\\').filter(|part| !part.is_empty());
    let Some(server) = parts.next() else {
        return normalized;
    };
    let rest: Vec<_> = parts.collect();
    if rest.is_empty() {
        format!("\\\\{server}\\")
    } else {
        format!("\\\\{}\\{}", server, rest.join("\\"))
    }
}

fn unc_server_name(path: &Path) -> Option<String> {
    let text = path.as_os_str().to_string_lossy();
    if !text.starts_with("\\\\") {
        return None;
    }
    text.trim_start_matches('\\')
        .split('\\')
        .find(|part| !part.is_empty())
        .map(str::to_string)
}

fn network_share_entries(path: &Path) -> Vec<Entry> {
    let Some(server) = unc_server_name(path) else {
        return Vec::new();
    };
    let mut server_wide: Vec<u16> = format!("\\\\{server}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut buffer: *mut u8 = ptr::null_mut();
    let mut entries_read = 0u32;
    let mut total_entries = 0u32;
    let status = unsafe {
        NetShareEnum(
            server_wide.as_mut_ptr(),
            1,
            &mut buffer,
            u32::MAX,
            &mut entries_read,
            &mut total_entries,
            ptr::null_mut(),
        )
    };
    if status != ERROR_SUCCESS && status != ERROR_ACCESS_DENIED || buffer.is_null() {
        return Vec::new();
    }
    let shares =
        unsafe { std::slice::from_raw_parts(buffer as *const SHARE_INFO_1, entries_read as usize) };
    let mut result = Vec::new();
    for share in shares {
        if share.shi1_netname.is_null() {
            continue;
        }
        if share.shi1_type & STYPE_MASK != STYPE_DISKTREE {
            continue;
        }
        let name = unsafe {
            let mut len = 0usize;
            while *share.shi1_netname.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(share.shi1_netname, len))
        };
        if name.ends_with('$') {
            continue;
        }
        let share_path = PathBuf::from(format!("\\\\{server}\\{name}"));
        result.push(Entry {
            name,
            path: share_path,
            is_dir: true,
            size: 0,
            modified: "网络共享".to_string(),
            modified_millis: 0,
        });
    }
    unsafe {
        NetApiBufferFree(buffer as *mut _);
    }
    result.sort_by_key(|entry| entry.name.to_lowercase());
    result
}

fn is_unc_server_root(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    if !text.starts_with("\\\\") {
        return false;
    }
    let parts: Vec<_> = text
        .trim_start_matches('\\')
        .split('\\')
        .filter(|part| !part.is_empty())
        .collect();
    parts.len() == 1
}

fn can_open_directory(path: &Path) -> bool {
    // UNC 服务器根：不在此处做任何网络 I/O（可能秒级阻塞）。read_entries
    // 在后台线程处理，根视图总会出结果（共享列表或提示条目）
    if is_unc_server_root(path) {
        return true;
    }
    path.is_dir() || fs::read_dir(path).is_ok()
}

const PATH_STATUS_DIR: u8 = 1;
const PATH_STATUS_OTHER: u8 = 2;

/// 渲染安全的目录判断：**渲染线程绝不能 stat**——侧边栏每帧都会对
/// Recents/Places/盘符做校验，NAS/掉线路径一次 SMB 往返就能把每次点击
/// 拖到秒级。命中缓存直接返回；未命中丢后台 stat，先返回 assume 假设值，
/// 验证完成后通过 UI_REFRESH_TX 通知重绘刷新。
fn cached_path_is_dir(path: &Path, assume: bool) -> bool {
    if let Some(status) = path_status_cached(path) {
        return status;
    }
    queue_path_status(path);
    assume
}

fn path_status_cached(path: &Path) -> Option<bool> {
    let map = PATH_STATUS.get_or_init(|| Mutex::new(HashMap::new()));
    let map = map.lock().ok()?;
    match map.get(path) {
        Some(&PATH_STATUS_DIR) => Some(true),
        Some(&PATH_STATUS_OTHER) => Some(false),
        _ => None,
    }
}

fn queue_path_status(path: &Path) {
    let pending = PATH_STATUS_PENDING.get_or_init(|| Mutex::new(HashSet::new()));
    let should_probe = pending
        .lock()
        .map(|mut pending| pending.insert(path.to_path_buf()))
        .unwrap_or(false);
    if !should_probe {
        return;
    }
    let probe = path.to_path_buf();
    thread::spawn(move || {
        // 慢/掉线路径阻塞的是这个后台线程，不卡 UI
        let is_dir = probe.is_dir();
        if let Ok(mut map) = PATH_STATUS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
            map.insert(probe.clone(), if is_dir { PATH_STATUS_DIR } else { PATH_STATUS_OTHER });
        }
        if let Ok(mut pending) = PATH_STATUS_PENDING
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
        {
            pending.remove(&probe);
        }
        if let Some(sender) = UI_REFRESH_TX.get() {
            let _ = sender.try_send(probe);
        }
    });
}

fn is_this_pc(path: &Path) -> bool {
    path.as_os_str().to_string_lossy() == THIS_PC_PATH
}

fn is_network_path(path: &Path) -> bool {
    path.as_os_str().to_string_lossy() == NETWORK_PATH
}

fn is_recycle_bin_path(path: &Path) -> bool {
    path.as_os_str().to_string_lossy() == RECYCLE_BIN_PATH
}

fn is_virtual_path(path: &Path) -> bool {
    is_this_pc(path) || is_network_path(path) || is_recycle_bin_path(path)
}

fn virtual_title(path: &Path) -> &'static str {
    if is_this_pc(path) {
        "此电脑"
    } else if is_network_path(path) {
        "网络"
    } else if is_recycle_bin_path(path) {
        "回收站"
    } else {
        "虚拟位置"
    }
}

fn virtual_entries(path: &Path) -> Vec<Entry> {
    if is_this_pc(path) {
        let mut entries = drive_entries();
        entries.push(Entry {
            name: "网络".to_string(),
            path: PathBuf::from(NETWORK_PATH),
            is_dir: true,
            size: 0,
            modified: "--".to_string(),
            modified_millis: 0,
        });
        entries.push(Entry {
            name: "回收站".to_string(),
            path: PathBuf::from(RECYCLE_BIN_PATH),
            is_dir: true,
            size: 0,
            modified: "--".to_string(),
            modified_millis: 0,
        });
        entries
    } else if is_network_path(path) {
        vec![Entry {
            name: "打开系统网络位置".to_string(),
            path: PathBuf::from(NETWORK_PATH),
            is_dir: true,
            size: 0,
            modified: "输入 \\\\192.168.x.x 可直达".to_string(),
            modified_millis: 0,
        }]
    } else if is_recycle_bin_path(path) {
        vec![Entry {
            name: "打开系统回收站".to_string(),
            path: PathBuf::from(RECYCLE_BIN_PATH),
            is_dir: true,
            size: 0,
            modified: "Windows Shell".to_string(),
            modified_millis: 0,
        }]
    } else {
        Vec::new()
    }
}

fn drive_entries() -> Vec<Entry> {
    available_drives()
        .into_iter()
        .map(|path| {
            let name = path.display().to_string();
            Entry {
                name,
                path,
                is_dir: true,
                size: 0,
                modified: "--".to_string(),
                modified_millis: 0,
            }
        })
        .collect()
}

fn breadcrumb_paths(path: &Path) -> Vec<(String, PathBuf)> {
    if is_virtual_path(path) {
        return vec![(virtual_title(path).to_string(), path.to_path_buf())];
    }
    let mut result = Vec::new();
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match component {
            std::path::Component::Prefix(prefix) => {
                result.push((
                    format!("{}\\", prefix.as_os_str().to_string_lossy()),
                    current.clone(),
                ));
            }
            std::path::Component::RootDir => {}
            std::path::Component::Normal(name) => {
                result.push((name.to_string_lossy().to_string(), current.clone()));
            }
            _ => {}
        }
    }
    if result.is_empty() {
        result.push((path.display().to_string(), path.to_path_buf()));
    }
    result
}

fn forward_breadcrumb_paths(current: &Path, forward: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let Some(target) = forward.last() else {
        return Vec::new();
    };
    let Ok(remainder) = target.strip_prefix(current) else {
        return Vec::new();
    };
    let mut path = current.to_path_buf();
    let mut result = Vec::new();
    for component in remainder.components() {
        if let std::path::Component::Normal(name) = component {
            path.push(name);
            result.push((name.to_string_lossy().to_string(), path.clone()));
        }
    }
    result
}

fn address_suggestions(value: &str) -> Vec<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        return ('C'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|path| path.is_dir())
            .collect();
    }
    if value.starts_with("//") || value.starts_with("\\\\") {
        let candidate = path_from_user_input(value);
        // UNC 服务器根/子路径的网络探测可能吃秒级超时，限时 1.2s 防止
        // 地址栏每敲一个字都卡一次；超时就给空建议
        if is_unc_server_root(&candidate) {
            let server = unc_server_name(&candidate).unwrap_or_default();
            if unc_enum_recently_denied(&server) {
                // 负缓存命中：直接回显已知共享，不再发起网络探测
                return known_unc_share_entries(&server)
                    .into_iter()
                    .map(|entry| entry.path)
                    .take(12)
                    .collect();
            }
            let shares = network_share_entries(&candidate);
            if !shares.is_empty() {
                return shares
                    .into_iter()
                    .map(|entry| entry.path)
                    .take(12)
                    .collect();
            }
            // 本函数在后台线程跑：read_dir 慢就慢（SMB 会话热时可成功），
            // 不设看门狗（超时掐掉会把"慢但成功"的完整列表变成"快速但空"）
            if let Ok(iterator) = fs::read_dir(&candidate) {
                let result: Vec<PathBuf> = iterator
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .take(12)
                    .collect();
                if !result.is_empty() {
                    return result;
                }
            }
            mark_unc_enum_denied(&server);
            return known_unc_share_entries(&server)
                .into_iter()
                .map(|entry| entry.path)
                .take(12)
                .collect();
        }
        if candidate.is_dir() || fs::read_dir(&candidate).is_ok() {
            return fs::read_dir(candidate)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .take(12)
                .collect();
        }
        return Vec::new();
    }
    if !value.contains(['\\', '/']) {
        return ('C'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|path| {
                path.is_dir()
                    && path
                        .display()
                        .to_string()
                        .to_lowercase()
                        .starts_with(&value.to_lowercase())
            })
            .collect();
    }
    let candidate = path_from_user_input(value);
    let (directory, prefix) = if candidate.is_dir() {
        (candidate, String::new())
    } else {
        (
            candidate
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("C:\\")),
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_lowercase(),
        )
    };
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.is_dir()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&prefix))
            .then_some(path)
        })
        .take(12)
        .collect()
}

fn available_drives() -> Vec<PathBuf> {
    // GetLogicalDrives 位掩码，零 I/O：原来逐字母 path.is_dir() 在渲染热路径
    // （侧边栏每帧调用），映射掉线的网络盘一次 stat 就能把点击拖到秒级。
    // 掉线映射盘符照样列出（资源管理器也这样），打开失败由导航层报状态。
    let mask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|letter| mask & (1u32 << letter) != 0)
        .map(|letter| PathBuf::from(format!("{}:\\", (b'A' + letter as u8) as char)))
        .collect()
}

fn nav_icon_path(name: &str) -> PathBuf {
    // 图标随 exe 分发：exe 同级 Assets\nav（便携发布 / 开发 target 目录旁挂均适用）。
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    exe_dir.join("Assets").join("nav").join(format!("{name}.png"))
}

fn folder_icon(size: f32) -> AnyElement {
    let body_height = size * 0.62;
    div()
        .relative()
        .w(px(size))
        .h(px(size * 0.76))
        .child(
            div()
                .absolute()
                .left(px(size * 0.08))
                .top(px(size * 0.04))
                .w(px(size * 0.47))
                .h(px(size * 0.25))
                .rounded_t_sm()
                .bg(rgb(0xf2bd38)),
        )
        .child(
            div()
                .absolute()
                .left(px(0.))
                .bottom(px(0.))
                .w(px(size))
                .h(px(body_height))
                .rounded_sm()
                .bg(rgb(0xffcf52))
                .border_1()
                .border_color(rgb(0xe2aa22)),
        )
        .into_any_element()
}

fn file_icon(size: f32) -> AnyElement {
    div()
        .relative()
        .w(px(size * 0.76))
        .h(px(size))
        .rounded_xs()
        .bg(rgb(0xf5f7fa))
        .border_1()
        .border_color(rgb(0x91a2b4))
        .child(
            div()
                .absolute()
                .left(px(size * 0.15))
                .right(px(size * 0.15))
                .top(px(size * 0.48))
                .h(px(1.))
                .bg(rgb(0x91a2b4)),
        )
        .child(
            div()
                .absolute()
                .left(px(size * 0.15))
                .right(px(size * 0.15))
                .top(px(size * 0.68))
                .h(px(1.))
                .bg(rgb(0x91a2b4)),
        )
        .into_any_element()
}

/// 小尺寸行内文件图标：优先系统关联图标（zip/exe/pdf 真实图标），回退通用白纸。
fn inline_file_icon(path: &Path, size: f32) -> AnyElement {
    if let Some(type_icon) = cached_filetype_icon_path(path, size) {
        let fallback_size = size;
        img(type_icon)
            .w(px(size * 0.82))
            .h(px(size * 0.82))
            .object_fit(ObjectFit::Contain)
            .with_fallback(move || file_icon(fallback_size))
            .into_any_element()
    } else {
        file_icon(size)
    }
}

fn shell_item_from_path(path: &Path) -> WResult<IShellItem> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { SHCreateItemFromParsingName(WPCWSTR(wide.as_ptr()), None::<&IBindCtx>) }
}

/// 从回收站恢复一批被删路径：枚举回收站找到原路径匹配项，对其执行
/// Shell "undo" 动词（与资源管理器 Ctrl+Z 相同通道）。
fn undo_recycle_restore(original_paths: &[PathBuf]) -> Result<(), String> {
    let wanted: HashSet<PathBuf> = original_paths
        .iter()
        .map(|path| normalize_path_buf(path))
        .collect();
    let (result_tx, result_rx) = mpsc::channel::<Result<(), String>>();
    let wanted_for_thread = wanted.clone();
    let worker = std::thread::Builder::new()
        .name("fileflow-undo-restore".into())
        .spawn(move || {
            let result = unsafe {
                let co_result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let should_uninitialize = co_result == S_OK;
                if co_result.is_err() && co_result != RPC_E_CHANGED_MODE {
                    Err(format!("COM 初始化失败：{co_result:?}"))
                } else {
                    let outcome =
                        restore_from_recycle_bin_sta(&wanted_for_thread);
                    if should_uninitialize {
                        CoUninitialize();
                    }
                    outcome
                }
            };
            let _ = result_tx.send(result);
        });
    match worker {
        Ok(_) => result_rx
            .recv()
            .unwrap_or_else(|_| Err("恢复线程异常退出".to_string())),
        Err(error) => Err(format!("启动恢复线程失败：{error}")),
    }
}

fn normalize_path_buf(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

unsafe fn restore_from_recycle_bin_sta(wanted: &HashSet<PathBuf>) -> Result<(), String> {
    use windows::core::HSTRING;
    unsafe {
        // 回收站根 folder item
        let recycle: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from("shell:RecycleBinFolder"), None)
                .map_err(|e| format!("打开回收站失败：{e}"))?;
        let folder: IShellFolder = recycle
            .BindToHandler(None, &windows::Win32::UI::Shell::BHID_SFObject)
            .map_err(|e| format!("绑定回收站失败：{e}"))?;
        let mut enum_opt: Option<windows::Win32::UI::Shell::IEnumIDList> = None;
        let flags = (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0) as u32;
        let hr = folder.EnumObjects(
            windows::Win32::Foundation::HWND::default(),
            flags,
            &mut enum_opt,
        );
        if hr.is_err() {
            return Err(format!("枚举回收站失败：0x{:08X}", hr.0 as u32));
        }
        let Some(enum_idlist) = enum_opt else {
            return Err("回收站枚举器为空".to_string());
        };
        let mut restored = 0usize;
        loop {
            let mut items: [*mut ITEMIDLIST; 8] = [ptr::null_mut(); 8];
            let mut fetched: u32 = 0;
            let hr = enum_idlist.Next(&mut items, Some(&mut fetched));
            if hr.is_err() || fetched == 0 {
                break;
            }
            for pidl in items.into_iter().take(fetched as usize) {
                if pidl.is_null() {
                    continue;
                }
                // $I 元数据里的原路径匹配才恢复
                let original = original_path_of_recycle_item(&folder, pidl);
                if let Some(original) = original
                    && wanted.contains(&normalize_path_buf(&original))
                    && invoke_recycle_undo(&folder, pidl).is_ok()
                {
                    restored += 1;
                }
                CoTaskMemFree(Some(pidl as _));
            }
        }
        if restored == 0 {
            Err("回收站中没有找到匹配项".to_string())
        } else {
            Ok(())
        }
    }
}

/// 回收站项的解析名（$I 文件路径）；读 $I 元数据里的原路径。
unsafe fn original_path_of_recycle_item(
    folder: &IShellFolder,
    pidl: *mut ITEMIDLIST,
) -> Option<PathBuf> {
    unsafe {
        let mut strret = windows::Win32::UI::Shell::Common::STRRET::default();
        folder
            .GetDisplayNameOf(pidl, SHGDN_FORPARSING, &mut strret)
            .ok()?;
        let parsing_str = string_from_pwstr(strret);
        let meta_path = PathBuf::from(&parsing_str);
        read_recycle_meta_original(&meta_path)
    }
}

/// $I 文件（回收站元数据，V2）布局：
/// [0..4) 版本=2  [4..12) 原文件大小  [12..20) 删除时间 FILETIME
/// [20..24) 原路径 UTF-16 字节长度（含结尾 0），之后是路径数据
unsafe fn read_recycle_meta_original(meta_path: &Path) -> Option<PathBuf> {
    let Ok(mut file) = fs::File::open(meta_path) else {
        return None;
    };
    let mut header = vec![0u8; 24];
    if file.read_exact(&mut header).is_err() {
        return None;
    }
    let version = u32::from_le_bytes(header[0..4].try_into().ok()?);
    if version != 2 {
        return None;
    }
    let path_len = u32::from_le_bytes(header[20..24].try_into().ok()?) as usize;
    if path_len == 0 || path_len > 32768 * 2 {
        return None;
    }
    let mut name_bytes = vec![0u8; path_len];
    if file.read_exact(&mut name_bytes).is_err() {
        return None;
    }
    let units: Vec<u16> = name_bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    if units.is_empty() {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&units)))
}

/// 对回收站项执行 Shell "undo" 动词（与资源管理器 Ctrl+Z 同通道）。
unsafe fn invoke_recycle_undo(folder: &IShellFolder, pidl: *mut ITEMIDLIST) -> Result<(), String> {
    unsafe {
        let child_pidls = [pidl as *const ITEMIDLIST];
        let context_menu: IContextMenu = folder
            .GetUIObjectOf(
                windows::Win32::Foundation::HWND::default(),
                &child_pidls,
                None,
            )
            .map_err(|e| e.to_string())?;
        // CMINVOKECOMMANDINFO 的 lpVerb 是 ANSI 字符串（Unicode 版在 EX 结构）
        let verb: Vec<u8> = b"undo\0".to_vec();
        let info = CMINVOKECOMMANDINFO {
            cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
            hwnd: windows::Win32::Foundation::HWND::default(),
            lpVerb: WPCSTR(verb.as_ptr()),
            nShow: WSW_SHOWNORMAL.0,
            ..Default::default()
        };
        context_menu
            .InvokeCommand(&info)
            .map_err(|e| format!("undo 调用失败：{e}"))
    }
}

/// 专用 STA 线程执行 IFileOperation：确认/进度对话框要求线程有消息泵，
/// gpui 后台线程没有 → 确认框弹不出、PerformOperations 死等或静默取消。
/// 这里新建 STA 线程并泵消息直到操作完成，模态对话框正常弹出。
fn perform_shell_file_operation(
    kind: ShellOperationKind,
    sources: &[PathBuf],
    target: Option<&Path>,
) -> Result<(), String> {
    let sources = sources.to_vec();
    let target = target.map(|path| path.to_path_buf());
    let (result_tx, result_rx) = mpsc::channel::<Result<(), String>>();
    let worker = std::thread::Builder::new()
        .name("fileflow-shell-op".into())
        .spawn(move || {
            let result = unsafe {
                let co_result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let should_uninitialize = co_result == S_OK;
                if co_result.is_err() && co_result != RPC_E_CHANGED_MODE {
                    Err(format!("COM 初始化失败：{co_result:?}"))
                } else {
                    let outcome = run_file_operation_sta(kind, &sources, target.as_deref());
                    if should_uninitialize {
                        CoUninitialize();
                    }
                    outcome
                }
            };
            let _ = result_tx.send(result);
            // PerformOperations 之后 Shell 可能还挂有异步窗口（如进度 UI 收尾），
            // 泵一小段时间消息让它们正常关闭
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_millis(600) {
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    let mut drained = false;
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        drained = true;
                        if msg.message == WM_QUIT {
                            break;
                        }
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    if drained {
                        continue;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });
    match worker {
        Ok(_) => result_rx
            .recv()
            .unwrap_or_else(|_| Err("文件操作线程异常退出".to_string())),
        Err(error) => Err(format!("启动文件操作线程失败：{error}")),
    }
}

unsafe fn run_file_operation_sta(
    kind: ShellOperationKind,
    sources: &[PathBuf],
    target: Option<&Path>,
) -> Result<(), String> {
    unsafe {
    (|| -> WResult<()> {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
        // 同目录复制粘贴不再弹"确认替换"：冲突时自动改名为 "xxx - 副本"
        //
        // 永久删除（Shift+Del）必须**清掉 FOF_ALLOWUNDO**：带这个 flag 时，
        // Shell 即使没有 FOFX_RECYCLEONDELETE 也会建立撤销记录，
        // 会出现"已永久删除但 Ctrl+Z 还能恢复"的诡异状态。
        let permanent = matches!(kind, ShellOperationKind::PermanentDelete);
        let flags = FILEOPERATION_FLAGS(
            FOF_NOCONFIRMMKDIR.0
                | FOF_RENAMEONCOLLISION.0
                | if permanent {
                    0
                } else {
                    FOF_ALLOWUNDO.0 | FOFX_ADDUNDORECORD.0
                }
                | if matches!(kind, ShellOperationKind::Delete) {
                    FOFX_RECYCLEONDELETE.0
                } else {
                    0
                },
        );
        operation.SetOperationFlags(flags)?;
        let hwnd = cached_or_find_fileflow_hwnd();
        if !hwnd.is_null() {
            operation.SetOwnerWindow(WHWND(hwnd as *mut _))?;
        }
        let destination = match target {
            Some(path) => Some(shell_item_from_path(path)?),
            None => None,
        };
        for source in sources {
            let item = shell_item_from_path(source)?;
            match kind {
                ShellOperationKind::Copy => operation.CopyItem(
                    &item,
                    destination.as_ref().expect("copy destination"),
                    WPCWSTR::null(),
                    None::<&IFileOperationProgressSink>,
                )?,
                ShellOperationKind::Move => operation.MoveItem(
                    &item,
                    destination.as_ref().expect("move destination"),
                    WPCWSTR::null(),
                    None::<&IFileOperationProgressSink>,
                )?,
                ShellOperationKind::Delete | ShellOperationKind::PermanentDelete => {
                    operation.DeleteItem(&item, None::<&IFileOperationProgressSink>)?
                }
            }
        }
        operation.PerformOperations()?;
        if operation.GetAnyOperationsAborted()?.as_bool() {
            return Err(WError::from_hresult(DRAGDROP_S_CANCEL));
        }
        Ok(())
    })()
    .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NewItemKind { Folder, TextFile }

impl NewItemKind {
    fn label(self) -> &'static str {
        match self { Self::Folder => "文件夹", Self::TextFile => "TXT 文件" }
    }
}

fn new_item_names(input: &str, kind: NewItemKind) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let name = line.trim();
        if name.is_empty() { continue; }
        let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if name == "." || name == ".." || name.ends_with('.') || reserved
            || name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) {
            return Err(format!("第 {} 行名称无效：{}", index + 1, name));
        }
        names.push(if kind == NewItemKind::TextFile && !name.to_ascii_lowercase().ends_with(".txt") { format!("{name}.txt") } else { name.to_owned() });
    }
    if names.is_empty() { return Err("请至少输入一个名称".to_string()); }
    Ok(names)
}

fn create_new_items(base: &Path, names: &[String], kind: NewItemKind) -> (Vec<PathBuf>, Vec<String>) {
    let mut created = Vec::new();
    let mut errors = Vec::new();
    for name in names {
        // create_new/create_dir 都不覆盖已有项目；编号后再原子创建。
        let mut result = None;
        for _ in 0..100 {
            let path = unique_path(&base.join(name));
            let attempt = match kind {
                NewItemKind::Folder => fs::create_dir(&path),
                NewItemKind::TextFile => fs::OpenOptions::new().write(true).create_new(true).open(&path).map(drop),
            };
            match attempt {
                Ok(()) => { created.push(path); result = Some(Ok(())); break; }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => { result = Some(Err(error.to_string())); break; }
            }
        }
        if let Some(Err(error)) = result { errors.push(format!("{name}：{error}")); }
        else if result.is_none() { errors.push(format!("{name}：重名冲突过多")); }
    }
    (created, errors)
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("项目");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    for number in 1..10_000 {
        let candidate = parent.join(format!("{stem} ({number}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// 批量重命名：按名称生成 `名称_001.ext`、`名称_002.ext`…
/// 目标已存在的跳过（不覆盖），扩展名保留各自原有值。
/// 返回 (成功的新路径列表, 失败原因列表)。
fn batch_rename_files(
    targets: &[PathBuf],
    base: &str,
    start: u32,
) -> (Vec<PathBuf>, Vec<String>) {
    let mut renamed = Vec::new();
    let mut errors = Vec::new();
    if base.is_empty() {
        errors.push("名称不能为空".to_string());
        return (renamed, errors);
    }
    let width = targets.len().to_string().len().max(3);
    for (index, target) in targets.iter().enumerate() {
        let number = start + index as u32;
        let extension = target
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        let Some(parent) = target.parent() else {
            errors.push(format!("无法取得父目录：{}", target.display()));
            continue;
        };
        let new_path = parent.join(format!("{base}_{number:0width$}{extension}", width = width));
        if new_path == *target {
            renamed.push(new_path);
            continue;
        }
        if new_path.exists() {
            errors.push(format!(
                "已存在同名文件：{}",
                new_path.file_name().unwrap_or_default().to_string_lossy()
            ));
            continue;
        }
        match fs::rename(target, &new_path) {
            Ok(()) => renamed.push(new_path),
            Err(error) => errors.push(format!(
                "{}：{error}",
                target.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    (renamed, errors)
}

fn tab_name(path: &Path) -> String {
    if is_virtual_path(path) {
        return virtual_title(path).to_string();
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("This PC")
        .to_string()
}
fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "tif" | "tiff"
            )
        })
}

fn thumbnail_cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("FileFlow")
        .join("thumbnails")
}

/// 后台生成时的缓存 key：与 cached_thumbnail_for_entry 同一 hash 组成
/// （path + size + modified_millis），保证读写两侧 key 一致
fn thumbnail_cache_path_for(source: &Path, size: u64, modified_millis: u64) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    size.hash(&mut hasher);
    modified_millis.hash(&mut hasher);
    thumbnail_cache_dir().join(format!("{:016x}.png", hasher.finish()))
}

/// 按扩展名提取系统文件关联图标（zip/exe/pdf 等显示各自真实图标），
/// 转成 PNG 缓存复用。同一 (扩展名, 尺寸桶) 只提取一次（会话级记忆）——
/// 渲染帧绝不能反复 stat/提取，大目录（几百个文件）每帧重提取会卡到秒级。
fn cached_filetype_icon_path(path: &Path, size: f32) -> Option<PathBuf> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if extension.is_empty() {
        return None;
    }
    let bucket = ((size / 32.0).ceil() as u32).clamp(1, 16);
    let memo_key = (filetype_icon_identity(&extension, path), bucket);
    let memo = FILETYPE_ICON_MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = memo.lock()
        && let Some(hit) = map.get(&memo_key)
    {
        return hit.clone();
    }
    let computed = filetype_icon_png(&extension, bucket, path);
    if let Ok(mut map) = memo.lock() {
        map.insert(memo_key, computed.clone());
    }
    computed
}

// LNK 的图标属于单个快捷方式，不能按扩展名共享（也避免复用旧通用缓存）。
fn filetype_icon_identity(extension: &str, path: &Path) -> String {
    if extension == "lnk" {
        format!("shortcut-v1:{}", path.to_string_lossy().to_lowercase())
    } else {
        extension.to_string()
    }
}
/// 磁盘缓存 + 提取。失败也由调用方记住（memo），本会话不重试。
fn filetype_icon_png(extension: &str, bucket: u32, path: &Path) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{
        SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_LINKOVERLAY, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
    };
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let mut hasher = DefaultHasher::new();
    filetype_icon_identity(extension, path).hash(&mut hasher);
    bucket.hash(&mut hasher);
    let cache_dir = thumbnail_cache_dir().join("filetype");
    let cache_path = cache_dir.join(format!("icon_{:016x}.png", hasher.finish()));
    if cache_path.is_file() {
        return Some(cache_path);
    }
    fs::create_dir_all(&cache_dir).ok()?;

    unsafe {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut info = SHFILEINFOW::default();
        // 快捷方式必须查询真实路径，让 Shell 解析目标/自定义图标。
        let flags = if extension == "lnk" {
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_LINKOVERLAY
        } else {
            SHGFI_ICON | SHGFI_USEFILEATTRIBUTES | SHGFI_LARGEICON
        };
        let result = SHGetFileInfoW(
            WPCWSTR(wide.as_mut_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let converted = icon_to_png_cache(info.hIcon, &cache_path);
        let _ = DestroyIcon(info.hIcon);
        converted
    }
}

/// HICON → PNG 缓存。32bpp 顶向下 DIB 提取 RGBA，透明像素 alpha=0。
/// 尺寸必须用 GetObjectW 查询：GetDIBits 的"查询头"调用（cLines=0）在
/// bmiHeader 预置 biBitCount=32 时会直接返回 0（实测），旧实现因此
/// 缓存永远写不进盘、每帧重新提取，是大目录卡顿的根源。
unsafe fn icon_to_png_cache(
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
    cache_path: &Path,
) -> Option<PathBuf> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetIconInfo;

    unsafe {
    let mut info = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
    if GetIconInfo(icon, &mut info).is_err() {
        return None;
    }
    let screen_dc = GetDC(None);
    let memory_dc = CreateCompatibleDC(Some(screen_dc));
    let mut bmp = BITMAP::default();
    let queried = GetObjectW(
        info.hbmColor.into(),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut _),
    );
    let width = bmp.bmWidth;
    let height = bmp.bmHeight;
    if queried <= 0 || width <= 0 || height <= 0 || width > 256 || height > 256 {
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        return None;
    }
    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let mut dib = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // 顶向下
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0 /* BI_RGB */,
            ..Default::default()
        },
        ..Default::default()
    };
    let copied = GetDIBits(
        memory_dc,
        info.hbmColor,
        0,
        height as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut dib,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(memory_dc);
    let _ = ReleaseDC(None, screen_dc);
    let _ = DeleteObject(info.hbmColor.into());
    let _ = DeleteObject(info.hbmMask.into());
    if copied == 0 {
        return None;
    }
    // BGRA → RGBA
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        if chunk[3] == 0 {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
        }
    }
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, pixels)?;
    let temporary = cache_path.with_extension("tmp.png");
    image::RgbaImage::save(&buffer, &temporary).ok()?;
    fs::rename(&temporary, cache_path).ok()?;
    Some(cache_path.to_path_buf())
    }
}

/// 渲染热路径专用：用 Entry 已带的元数据（size + modified_millis，目录加载时
/// 一次拿到）算缓存 key，完全不碰文件系统。网络目录上每次 fs::metadata 都是
/// 一次 SMB 往返，逐条目 stat 会把整帧渲染拖到秒级。
/// 返回 Some(路径) = 缓存 PNG 可直接显示；None = 需要排队后台生成。
fn cached_thumbnail_for_entry(source: &Path, size: u64, modified_millis: u64) -> Option<PathBuf> {
    let cache_path = thumbnail_cache_path_for(source, size, modified_millis);
    if thumbnail_cache_known(&cache_path) {
        return Some(cache_path);
    }
    queue_thumbnail_generation(source);
    None
}

/// 渲染帧不反复 stat：首次查询记入会话记忆，后台生成成功后置 true
fn thumbnail_cache_known(cache_path: &Path) -> bool {
    let memo = THUMBNAIL_CACHE_MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = memo.lock() {
        if let Some(known) = map.get(cache_path) {
            return *known;
        }
        let hit = cache_path.is_file();
        map.insert(cache_path.to_path_buf(), hit);
        return hit;
    }
    cache_path.is_file()
}

fn mark_thumbnail_cached(cache_path: &Path) {
    if let Ok(mut map) = THUMBNAIL_CACHE_MEMO
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        map.insert(cache_path.to_path_buf(), true);
    }
}

/// 排队后台生成缩略图（与 cached_thumbnail_for_entry 共用 pending 去重）。
/// 生成失败过的路径进入 failed 黑名单（本会话内不再重排），
/// 否则损坏图片/不可达网络文件每帧都会重新排队，worker 永动、CPU 常驻。
fn queue_thumbnail_generation(source: &Path) {
    if let Ok(failed) = THUMBNAIL_FAILED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        && failed.contains(source)
    {
        return;
    }
    let pending = THUMBNAIL_PENDING.get_or_init(|| Mutex::new(HashSet::new()));
    let should_queue = pending
        .lock()
        .map(|mut pending| pending.insert(source.to_path_buf()))
        .unwrap_or(false);
    if should_queue
        && let Some(sender) = THUMBNAIL_REQUEST_TX.get()
        && sender.try_send(source.to_path_buf()).is_err()
        && let Ok(mut pending) = pending.lock()
    {
        pending.remove(source);
    }
}

fn generate_cached_thumbnail(source: &Path) -> Option<PathBuf> {
    // 后台生成：此处 stat 一次拿元数据（在 worker 线程，不卡 UI）
    let metadata = fs::metadata(source).ok()?;
    let modified_millis = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| {
            (duration.as_secs() + 11_644_473_600) * 10_000_000 + duration.subsec_nanos() as u64 / 100
        })
        .unwrap_or(0);
    let cache_path = thumbnail_cache_path_for(source, metadata.len(), modified_millis);
    if cache_path.is_file() {
        mark_thumbnail_cached(&cache_path);
        return Some(cache_path);
    }

    fs::create_dir_all(thumbnail_cache_dir()).ok()?;
    // 优先 image crate 直接解码（jpeg/png/webp/gif/bmp/tiff 全支持，webp 实测可解），
    // 失败再走 Windows Shell 真缩略图管线（覆盖装了预览扩展的 heic/avif/psd 等格式）。
    // 注意：不能用会回退图标的 shell 模式——那会把文件类型图标当成缩略图缓存。
    let decoded = image::ImageReader::open(source)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.decode().ok())
        .or_else(|| shell_thumbnail(source))
        .map(|image| image.thumbnail(256, 256))?;
    let temporary = cache_path.with_extension("tmp.png");
    decoded.save_with_format(&temporary, image::ImageFormat::Png).ok()?;
    fs::rename(&temporary, &cache_path).ok()?;
    mark_thumbnail_cached(&cache_path);
    Some(cache_path)
}

/// 用 Windows Shell (IShellItemImageFactory) 生成缩略图，走系统缓存与系统解码器。
/// 需要 COM；在未初始化 COM 的线程上会失败并返回 None，由调用方回退 image crate。
fn shell_thumbnail(source: &Path) -> Option<image::DynamicImage> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits,
    };
    use windows::Win32::UI::Shell::{SIIGBF_RESIZETOFIT, SIIGBF_THUMBNAILONLY};

    let co_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let should_uninitialize = co_result == S_OK;
    if co_result.is_err() && co_result != RPC_E_CHANGED_MODE {
        return None;
    }

    let outcome = (|| -> Option<image::DynamicImage> {
        let mut source_wide: Vec<u16> = source.display().to_string().encode_utf16().collect();
        source_wide.push(0);
        let factory: windows::Win32::UI::Shell::IShellItemImageFactory = unsafe {
            SHCreateItemFromParsingName(WPCWSTR(source_wide.as_ptr()), None::<&IBindCtx>)
                .ok()?
        };
        let hbitmap = unsafe {
            factory
                .GetImage(
                    SIZE {
                        cx: 256,
                        cy: 256,
                    },
                    // THUMBNAILONLY：只收系统真缩略图，拒绝文件类型图标兜底
                    SIIGBF_RESIZETOFIT | SIIGBF_THUMBNAILONLY,
                )
                .ok()?
        };

        // HBITMAP -> BGRA 像素 -> RGBA DynamicImage
        unsafe {
            let hdc = GetDC(None);
            if hdc.is_invalid() {
                let _ = DeleteObject(hbitmap.into());
                return None;
            }
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<
                windows::Win32::Graphics::Gdi::BITMAPINFOHEADER,
            >() as u32;
            let probed = GetDIBits(
                hdc,
                hbitmap,
                0,
                0,
                None,
                &mut info,
                DIB_RGB_COLORS,
            );
            if probed == 0 || info.bmiHeader.biWidth <= 0 || info.bmiHeader.biHeight == 0 {
                let _ = DeleteDC(hdc);
                let _ = DeleteObject(hbitmap.into());
                return None;
            }
            let width = info.bmiHeader.biWidth as u32;
            // GetDIBits 返回自底向上（正高度）；置负高度让它按顶向下排列。
            // 高度必须先取绝对值再转 u32，否则 -256 as u32 会得到 4294967040（分配爆内存）。
            let height = info.bmiHeader.biHeight.unsigned_abs();
            info.bmiHeader.biHeight = -(height as i32);
            let mut pixels: Vec<u8> = vec![0u8; (width as usize) * (height as usize) * 4];
            let copied = GetDIBits(
                hdc,
                hbitmap,
                0,
                height,
                Some(pixels.as_mut_ptr() as *mut core::ffi::c_void),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(hdc);
            let _ = DeleteObject(hbitmap.into());
            if copied == 0 {
                return None;
            }
            // BGRA -> RGBA
            for chunk in pixels.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            let buffer = image::RgbaImage::from_raw(width, height, pixels)?;
            Some(image::DynamicImage::ImageRgba8(buffer))
        }
    })();

    if should_uninitialize {
        unsafe { CoUninitialize() };
    }
    outcome
}

fn start_thumbnail_worker() -> async_channel::Receiver<PathBuf> {
    let (request_tx, request_rx) = async_channel::bounded::<PathBuf>(256);
    let (result_tx, result_rx) = async_channel::bounded::<PathBuf>(256);
    let _ = THUMBNAIL_REQUEST_TX.set(request_tx);
    thread::spawn(move || {
        while let Ok(source) = request_rx.recv_blocking() {
            let generated = generate_cached_thumbnail(&source).is_some();
            if let Ok(mut pending) = THUMBNAIL_PENDING
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
            {
                pending.remove(&source);
            }
            if !generated {
                // 失败：进入黑名单，本会话不再重排（防止每帧重排的 CPU 风暴）
                if let Ok(mut failed) = THUMBNAIL_FAILED
                    .get_or_init(|| Mutex::new(HashSet::new()))
                    .lock()
                {
                    failed.insert(source);
                }
                continue;
            }
            let _ = result_tx.send_blocking(source);
        }
    });
    result_rx
}

/// 启动文件夹大小计算 worker：后台递归求和写缓存，完成发刷新信号。
/// 结果泵复用 start_thumbnail_result_pump 的节流（PathBuf 通道）——
/// 发的 PathBuf 是哨兵值，泵只负责触发 notify。
fn start_folder_size_worker() -> async_channel::Receiver<PathBuf> {
    let (req_tx, req_rx) = async_channel::bounded::<PathBuf>(256);
    let (done_tx, done_rx) = async_channel::bounded::<PathBuf>(8);
    let _ = FOLDER_SIZE_TX.set(req_tx);
    let done_tx_clone = done_tx.clone();
    let _ = FOLDER_SIZE_DONE_TX.set(done_tx_clone);
    thread::spawn(move || {
        let mut inflight: HashSet<PathBuf> = HashSet::new();
        while let Ok(dir) = req_rx.recv_blocking() {
            if !inflight.insert(dir.clone()) {
                continue; // 已在算
            }
            let total = recursive_dir_size(&dir, 0);
            FOLDER_SIZE_CACHE
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .map(|mut cache| cache.insert(dir.clone(), total))
                .ok();
            inflight.remove(&dir);
            let _ = done_tx.send_blocking(dir);
        }
    });
    done_rx
}

/// 递归求和（同步、后台线程跑）。深度上限 64 防 junction 环。
fn recursive_dir_size(dir: &Path, depth: usize) -> u64 {
    if depth > 64 {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue; // 不跟链接（防环 + 不重复计数）
            } else if ft.is_dir() {
                total += recursive_dir_size(&entry.path(), depth + 1);
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// 渲染用：文件夹大小（缓存命中返回 Some；未命中投递后台计算并返回 None）
fn folder_size_or_request(dir: &Path) -> Option<u64> {
    let cache = FOLDER_SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(size) = map.get(dir)
    {
        return Some(*size);
    }
    if let Some(tx) = FOLDER_SIZE_TX.get() {
        let _ = tx.try_send(dir.to_path_buf());
    }
    None
}

fn trim_thumbnail_cache() {
    const MAX_CACHE_FILES: usize = 512;
    const MAX_CACHE_BYTES: u64 = 192 * 1024 * 1024;

    let cache_dir = thumbnail_cache_dir();
    let Ok(entries) = fs::read_dir(&cache_dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then_some((
                entry.path(),
                metadata.len(),
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ))
        })
        .collect();
    files.sort_by_key(|(_, _, modified)| std::cmp::Reverse(*modified));
    let mut retained_bytes = 0u64;
    for (index, (path, size, _)) in files.into_iter().enumerate() {
        retained_bytes = retained_bytes.saturating_add(size);
        if index >= MAX_CACHE_FILES || retained_bytes > MAX_CACHE_BYTES {
            let _ = fs::remove_file(path);
        }
    }
}
fn type_label(entry: &Entry) -> String {
    if entry.is_dir {
        return "文件夹".to_string();
    }
    let extension = entry
        .path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension.is_empty() {
        return "文件".to_string();
    }
    if is_image_file(&entry.path) {
        format!("图像 ({extension}) 文件")
    } else {
        format!("{} 文件", extension.to_ascii_uppercase())
    }
}

fn type_counts(entries: &[Entry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(type_label(entry)).or_insert(0) += 1;
    }
    counts
}

fn toggle_native_maximize(window: &mut Window) {
    unsafe {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            let command = if IsZoomed(hwnd) != 0 {
                SW_RESTORE
            } else {
                SW_MAXIMIZE
            };
            let _ = ShowWindowAsync(hwnd, command);
        } else {
            window.zoom_window();
        }
    }
}

fn focus_fileflow_window() {
    let hwnd = cached_or_find_fileflow_hwnd();
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let foreground = GetForegroundWindow();
        let foreground_thread = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, ptr::null_mut())
        };
        let current_thread = GetCurrentThreadId();
        if foreground_thread != 0 && foreground_thread != current_thread {
            let _ = AttachThreadInput(current_thread, foreground_thread, 1);
        }
        // 只在最小化或隐藏到托盘时才恢复（SW_RESTORE 对最大化窗口的语义是
        // "取消最大化"会把全屏窗口变小——F2 重命名置前时绝不能碰最大化状态）。
        // 关闭到托盘 = SW_HIDE：IsIconic 为假但窗口不可见，SetForegroundWindow
        // 对隐藏窗口无效，必须先 Show。
        if IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        SwitchToThisWindow(hwnd, 1);
        if foreground_thread != 0 && foreground_thread != current_thread {
            let _ = AttachThreadInput(current_thread, foreground_thread, 0);
        }
    }
}

fn hide_fileflow_to_tray() {
    cache_fileflow_hwnd();
    let hwnd = cached_or_find_fileflow_hwnd();
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn wide_null(text: &str) -> Vec<u16> {
    let mut value: Vec<u16> = text.encode_utf16().collect();
    value.push(0);
    value
}

fn shell_open_default(path: &Path) -> bool {
    shell_execute_verb(path, "open")
}

fn shell_execute_verb(path: &Path, verb: &str) -> bool {
    let operation = wide_null(verb);
    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    result as isize > 32
}

fn write_wide_fixed(target: &mut [u16], text: &str) {
    target.fill(0);
    for (slot, value) in target.iter_mut().zip(text.encode_utf16()) {
        *slot = value;
    }
}

unsafe fn tray_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let icon = unsafe { LoadIconW(instance, 1 as windows_sys::core::PCWSTR) };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_FILEFLOW_TRAY;
    data.hIcon = if icon.is_null() {
        unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) }
    } else {
        icon
    };
    write_wide_fixed(&mut data.szTip, "FileFlow");
    data
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let mut data = unsafe { tray_icon_data(hwnd) };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let data = unsafe { tray_icon_data(hwnd) };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    // v4 格式下右键可能同时投递 WM_RBUTTONUP 与 WM_CONTEXTMENU，
    // TrackPopupMenu 是模态循环，重入会弹第二个菜单，用互斥挡掉
    if TRAY_MENU_OPEN.swap(true, AtomicOrdering::Relaxed) {
        return;
    }
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        TRAY_MENU_OPEN.store(false, AtomicOrdering::Relaxed);
        return;
    }
    let open = wide_null("打开");
    let exit = wide_null("退出");
    unsafe {
        let _ = AppendMenuW(menu, MF_STRING, TRAY_OPEN_ID, open.as_ptr());
        let _ = AppendMenuW(menu, MF_STRING, TRAY_EXIT_ID, exit.as_ptr());
        let mut point = POINT { x: 0, y: 0 };
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            ptr::null(),
        );
        let _ = DestroyMenu(menu);
        // 经典收尾：让菜单失去前台后能正确消失
        let _ = PostMessageW(hwnd, 0, 0, 0);
        integration_log(&format!("tray menu command={command}"));
        match command as usize {
            TRAY_OPEN_ID => focus_fileflow_window(),
            TRAY_EXIT_ID => {
                remove_tray_icon(hwnd);
                std::process::exit(0);
            }
            _ => {}
        }
        TRAY_MENU_OPEN.store(false, AtomicOrdering::Relaxed);
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_FILEFLOW_TRAY => {
            // 唤起指令（Win+E 钩子转发）：wparam=0、lparam=0x10000 精确匹配。
            // 不能用"lparam>>16==1"判断——VERSION_4 托盘事件的 lparam 是
            // 消息|(uID<<16)，uID=1 会让每个事件（含悬停）都被误判成唤起，
            // 这正是旧版"滑到托盘自动弹窗口、右键不出菜单"的根因（10-02 实测定位）。
            if wparam == 0 && lparam as u32 == 0x10000 {
                focus_fileflow_window();
                return 0;
            }
            // NIM_SETVERSION(VERSION_4) 后实测：wparam = 光标坐标（LO=x, HI=y），
            // lparam = 鼠标消息 | (uID << 16)；旧格式则是 wparam=uID、lparam=消息。
            // 两种格式下消息都在 lparam 低 16 位，统一取。
            let event = (lparam as u32) & 0xffff;
            integration_log(&format!(
                "tray event=0x{event:04x} wparam=0x{wparam:x} lparam=0x{lparam:x}"
            ));
            match event {
                // 右键：v4 发 WM_CONTEXTMENU（可能与 WM_RBUTTONUP 连发，菜单内部互斥）
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    unsafe { show_tray_menu(hwnd) };
                    0
                }
                // 左键只认双击唤起：单击（NIN_SELECT）/ 悬停滑过（WM_MOUSEMOVE、
                // NIN_POPUPOPEN=0x0406、NIN_POPUPCLOSE=0x0407 等）一律忽略
                WM_LBUTTONDBLCLK => {
                    focus_fileflow_window();
                    0
                }
                _ => 0,
            }
        }
        WM_DESTROY => {
            unsafe {
                remove_tray_icon(hwnd);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn start_tray_listener() {
    if TRAY_LISTENER_STARTED.swap(true, AtomicOrdering::Relaxed) {
        return;
    }
    thread::spawn(|| unsafe {
        let class_name = wide_null("FileFlowTrayWindow");
        let instance = GetModuleHandleW(ptr::null());
        let wnd_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(tray_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        let _ = RegisterClassW(&wnd_class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide_null("FileFlow Tray").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        add_tray_icon(hwnd);
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    });
}

unsafe extern "system" fn enum_own_fileflow_windows(hwnd: HWND, lparam: LPARAM) -> i32 {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == unsafe { GetCurrentProcessId() } {
        let result = lparam as *mut isize;
        if !result.is_null() {
            unsafe { *result = hwnd as isize };
        }
        return 0;
    }
    1
}

unsafe extern "system" fn enum_titled_fileflow_windows(hwnd: HWND, lparam: LPARAM) -> i32 {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }
    let mut buffer = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if len <= 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&buffer[..len as usize]);
    if title.trim() == "FileFlow" {
        let result = lparam as *mut isize;
        if !result.is_null() {
            unsafe { *result = hwnd as isize };
        }
        return 0;
    }
    1
}

fn cache_fileflow_hwnd() {
    if let Some(hwnd) = find_own_fileflow_window() {
        FILEFLOW_HWND.store(hwnd as isize, AtomicOrdering::Relaxed);
    }
}

fn find_own_fileflow_window() -> Option<HWND> {
    let mut found = 0isize;
    unsafe {
        let _ = EnumWindows(
            Some(enum_own_fileflow_windows),
            &mut found as *mut isize as LPARAM,
        );
    }
    if found == 0 {
        None
    } else {
        Some(found as HWND)
    }
}

fn find_titled_fileflow_window() -> Option<HWND> {
    let mut found = 0isize;
    unsafe {
        let _ = EnumWindows(
            Some(enum_titled_fileflow_windows),
            &mut found as *mut isize as LPARAM,
        );
    }
    if found == 0 {
        None
    } else {
        Some(found as HWND)
    }
}

fn cached_or_find_fileflow_hwnd() -> HWND {
    let cached = FILEFLOW_HWND.load(AtomicOrdering::Relaxed);
    if cached != 0 {
        return cached as HWND;
    }
    if let Some(hwnd) = find_own_fileflow_window() {
        FILEFLOW_HWND.store(hwnd as isize, AtomicOrdering::Relaxed);
        return hwnd;
    }
    if let Some(hwnd) = find_titled_fileflow_window() {
        return hwnd;
    }
    let mut title: Vec<u16> = "FileFlow".encode_utf16().collect();
    title.push(0);
    unsafe { FindWindowW(ptr::null(), title.as_ptr()) }
}

fn clean_external_path_arg(text: &str) -> Option<PathBuf> {
    let mut value = text.trim().trim_matches('"').trim_matches('\'').to_string();
    if value.is_empty() {
        return None;
    }
    if value.starts_with("file:///") {
        value = value.trim_start_matches("file:///").replace('/', "\\");
    }
    if value.starts_with("file://") {
        value = value.trim_start_matches("file://").replace('/', "\\");
    }
    if value.starts_with('/') && value.len() >= 3 && value.as_bytes().get(2) == Some(&b':') {
        value.remove(0);
    }
    // 盘符路径里的正斜杠/重复斜杠统一为 Windows 形式，避免 SHParseDisplayName 0x80070057
    if value.as_bytes().get(1) == Some(&b':') && !value.starts_with("\\\\") {
        let drive = &value[..2];
        let rest = value[2..].replace('/', "\\");
        let mut normalized = String::from(drive);
        let mut previous_was_separator = false;
        for character in rest.chars() {
            if character == '\\' {
                if !previous_was_separator {
                    normalized.push(character);
                }
                previous_was_separator = true;
            } else {
                normalized.push(character);
                previous_was_separator = false;
            }
        }
        value = normalized;
    }
    Some(PathBuf::from(value))
}

fn external_path_from_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].trim();
        let lower = arg.to_ascii_lowercase();
        let normalized = lower.replace('/', "\\");
        if normalized.ends_with("\\explorer.exe") || normalized == "explorer.exe" {
            index += 1;
            continue;
        }
        if lower == "/select" || lower == "-select" {
            if let Some(next) = args.get(index + 1) {
                return clean_external_path_arg(next);
            }
        } else if lower.starts_with("/select,") || lower.starts_with("-select,") {
            if let Some((_, path)) = arg.split_once(',') {
                return clean_external_path_arg(path);
            }
        } else if !arg.starts_with('/') && !arg.starts_with('-') {
            return clean_external_path_arg(arg);
        }
        index += 1;
    }
    None
}

fn send_external_path_to_existing(path: &Path) -> bool {
    let Ok(mut stream) = TcpStream::connect(FILEFLOW_COMMAND_ADDR) else {
        return false;
    };
    let text = path.display().to_string();
    stream.write_all(text.as_bytes()).is_ok()
}

#[derive(Debug)]
enum ExternalCommand {
    Path(PathBuf),
    Menu { command: &'static str, side: &'static str },
}

fn parse_external_command(text: &str) -> Option<ExternalCommand> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("fileflow-menu:") {
        let mut parts = rest.split(':');
        let command: &'static str = menu_token_to_static(parts.next()?)?;
        let side: &'static str = match parts.next().unwrap_or("left") {
            "right" => "right",
            _ => "left",
        };
        return Some(ExternalCommand::Menu { command, side });
    }
    clean_external_path_arg(trimmed).map(ExternalCommand::Path)
}

/// 只有已知的 token 才允许变成 'static str 分发（含子菜单带参数形式）
fn menu_token_to_static(raw: &str) -> Option<&'static str> {
    const TOKENS: [&str; 26] = [
        "open", "open-other", "copy", "cut", "paste", "rename", "delete", "new-folder",
        "new-text", "copy-path", "properties", "favorite", "explorer", "reveal", "terminal",
        "refresh", "view-details", "view-list", "view-columns", "view-m-icons", "view-l-icons",
        "view-xl-icons", "sort-name", "sort-type", "sort-modified", "sort-size",
    ];
    TOKENS.into_iter().find(|token| *token == raw)
}

fn start_external_command_listener() -> async_channel::Receiver<ExternalCommand> {
    let (tx, rx) = async_channel::unbounded();
    thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(FILEFLOW_COMMAND_ADDR) else {
            return;
        };
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = String::new();
            if stream.read_to_string(&mut buffer).is_ok()
                && let Some(command) = parse_external_command(&buffer)
            {
                let _ = tx.send_blocking(command);
            }
        }
    });
    rx
}

/// 等待 OLE 模态循环（DoDragDrop / Shell 菜单）退出后再 view.update。
/// timer 等待不触碰 App 借用，本身不会 panic；模态期间泵进来的后台任务
/// 在此挂起，循环结束后继续，避免 RefCell 重入。
async fn wait_out_of_ole_modal(cx: &AsyncApp) {
    while OLE_MODAL_ACTIVE.load(AtomicOrdering::Relaxed) {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
}

fn start_external_command_pump(
    window: gpui::WindowHandle<FileFlowGpui>,
    view: Entity<FileFlowGpui>,
    receiver: async_channel::Receiver<ExternalCommand>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        while let Ok(command) = receiver.recv().await {
            match command {
                ExternalCommand::Path(path) => {
                    wait_out_of_ole_modal(cx).await;
                    wait_out_of_ole_modal(cx).await;
                    let _ = view.update(cx, |this, cx| {
                        this.open_external_path(path, cx);
                    });
                    focus_fileflow_window();
                }
                ExternalCommand::Menu { command, side } => {
                    wait_out_of_ole_modal(cx).await;
                    let _ = window.update(cx, |this, window, cx| {
                        this.execute_menu_command(command, side, window, cx);
                    });
                    focus_fileflow_window();
                }
            }
        }
    })
    .detach();
}

fn start_directory_watcher() -> (mpsc::Sender<WatchCommand>, async_channel::Receiver<PathBuf>) {
    let (command_tx, command_rx) = mpsc::channel::<WatchCommand>();
    let (changed_tx, changed_rx) = async_channel::bounded::<PathBuf>(128);
    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel();
        let watcher_result: notify::Result<RecommendedWatcher> =
            notify::recommended_watcher(move |event| {
                let _ = event_tx.send(event);
            });
        let Ok(mut watcher) = watcher_result else {
            return;
        };
        let mut watched = HashSet::<PathBuf>::new();
        loop {
            match command_rx.recv_timeout(Duration::from_millis(80)) {
                Ok(WatchCommand::SetPaths(paths)) => {
                    let requested: HashSet<_> =
                        paths.into_iter().filter(|path| path.is_dir()).collect();
                    for old in watched.difference(&requested) {
                        let _ = watcher.unwatch(old);
                    }
                    for new_path in requested.difference(&watched) {
                        let _ = watcher.watch(new_path, RecursiveMode::NonRecursive);
                    }
                    watched = requested;
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }

            while let Ok(event) = event_rx.try_recv() {
                if let Ok(event) = event {
                    let changed = event
                        .paths
                        .first()
                        .and_then(|path| path.parent().map(Path::to_path_buf))
                        .or_else(|| watched.iter().next().cloned());
                    if let Some(path) = changed {
                        let _ = changed_tx.try_send(path);
                    }
                }
            }
        }
    });
    (command_tx, changed_rx)
}

fn start_directory_change_pump(
    view: Entity<FileFlowGpui>,
    receiver: async_channel::Receiver<PathBuf>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        while receiver.recv().await.is_ok() {
            cx.background_executor()
                .timer(Duration::from_millis(140))
                .await;
            while receiver.try_recv().is_ok() {}
            wait_out_of_ole_modal(cx).await;
            wait_out_of_ole_modal(cx).await;
            let _ = view.update(cx, |this, cx| this.reload_visible_async(cx));
        }
    })
    .detach();
}

fn start_thumbnail_result_pump(
    view: Entity<FileFlowGpui>,
    receiver: async_channel::Receiver<PathBuf>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        // 节流：每 150ms 至多一次整窗重绘。缩略图批量生成时若逐个
        // notify，会变成每张图一帧全量渲染，UI 直接卡死
        let executor = cx.background_executor().clone();
        let mut last_notify = Instant::now() - Duration::from_millis(200);
        while receiver.recv().await.is_ok() {
            while receiver.try_recv().is_ok() {}
            let elapsed = last_notify.elapsed();
            if elapsed < Duration::from_millis(150) {
                executor.timer(Duration::from_millis(150) - elapsed).await;
            }
            last_notify = Instant::now();
            wait_out_of_ole_modal(cx).await;
            let _ = view.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

fn claim_single_instance_or_focus_existing(external_path: Option<&Path>) -> bool {
    let mut name: Vec<u16> = "Local\\FileFlow.SingleInstance".encode_utf16().collect();
    name.push(0);
    unsafe {
        let handle = CreateMutexW(ptr::null_mut(), 1, name.as_ptr());
        if handle.is_null() {
            if let Some(path) = external_path {
                let _ = send_external_path_to_existing(path);
            }
            focus_fileflow_window();
            return false;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Some(path) = external_path {
                let _ = send_external_path_to_existing(path);
            }
            focus_fileflow_window();
            return false;
        }
    }
    true
}

unsafe extern "system" fn win_e_keyboard_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32
        && WIN_E_ENABLED.load(AtomicOrdering::Relaxed)
        && (wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN)
    {
        let key = unsafe { *(lparam as *const KBDLLHOOKSTRUCT) };
        let left_win_down = unsafe { GetAsyncKeyState(VK_LWIN as i32) } & 0x8000u16 as i16 != 0;
        let right_win_down = unsafe { GetAsyncKeyState(VK_RWIN as i32) } & 0x8000u16 as i16 != 0;
        if key.vkCode == VK_E as u32 && (left_win_down || right_win_down) {
            // 钩子回调里绝不调用 AttachThreadInput/SetForegroundWindow 这类会发
            // 同步跨线程消息的 API：钩子挂在全局，目标窗口过程若在 gpui 借用中，
            // 同步消息会把后台任务泵进来 → RefCell 重入 panic 且无法 unwind。
            // 只用完全异步的 ShowWindowAsync + SwitchToThisWindow(flash=0 不抢前台)，
            // 再 PostMessage 让 FileFlow 自己在前台安全地把窗口带上来。
            unsafe {
                let hwnd = cached_or_find_fileflow_hwnd();
                if !hwnd.is_null() {
                    let _ = ShowWindowAsync(hwnd, SW_RESTORE);
                    SwitchToThisWindow(hwnd, 0);
                    // lparam 高位=1：托盘窗口过程识别为"唤起"指令，
                    // 在托盘线程安全执行 focus_fileflow_window
                    let _ = PostMessageW(hwnd, WM_FILEFLOW_TRAY, 0, 0x10000);
                }
            }
            return 1;
        }
    }
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

/// subclass 窗口过程：标签行标题带 = 阈值拖动 + 点击透传，其余走 gpui 原过程。
///
/// 根因（10-02 修复"还是拖动不了"）：gpui 把 WM_NCLBUTTONDOWN 转成应用 MouseDown，
/// 一旦被应用消费就返回 Some(0) 不再走 DefWindowProc，系统拖动循环永远不启动；
/// 而直接进 DefWindowProc 又是"按下即拖"，标签/按钮点击全废。另外旧版命中带
/// y∈[36,80] 实际压在导航行（地址栏/按钮）上，标签行反而不在带内。
/// 因此改为：命中带 = 标签行整行；按下先吞掉缓存，移动超 4px 才拖动，
/// 否则松手把 down+up 合成客户区点击转发给 gpui。
unsafe extern "system" fn drag_zone_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCHITTEST {
            // lparam 屏幕坐标 → 窗口坐标
            let screen_x = (lparam as i32) & 0xffff;
            let screen_y = ((lparam as u32) >> 16) as i32;
            // 处理负坐标（多显示器）
            let screen_x = if screen_x >= 0x8000 { screen_x - 0x10000 } else { screen_x };
            let screen_y = if screen_y >= 0x8000 { screen_y - 0x10000 } else { screen_y };
            let mut rect: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
            if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect) != 0
                && screen_x >= rect.left
                && screen_x < rect.right
                && screen_y >= rect.top
                && screen_y < rect.bottom
            {
                // 标题带 = 标签行 0~42px（split 模式两个面板的 tab 行同样在此带）。
                // y 从 8px 起：顶部几 px 留给窗口缩放边（HTTOP/角）；
                // x 排除左侧 ☰ 按钮区（前 40px）与右侧窗口控制按钮（后 160px）。
                let scale = f32::from_bits(DRAG_SCALE.load(AtomicOrdering::Relaxed));
                let x = screen_x - rect.left;
                let y = screen_y - rect.top;
                let top = (8.0 * scale) as i32;
                let bottom = (42.0 * scale) as i32;
                let left_limit = (40.0 * scale) as i32;
                let right_limit = rect.right - rect.left - (160.0 * scale) as i32;
                if y >= top && y < bottom && x > left_limit && x < right_limit {
                    return HTCAPTION as _;
                }
            }
        } else if message == WM_NCLBUTTONDBLCLK && wparam == HTCAPTION as usize {
            // 双击标题带：最大化/还原（等价原生标题栏 DefWindowProc 的行为）。
            // 抬起用 CAPTION_SKIP_UP 吞掉，不给 gpui 孤立的 MouseUp。
            CAPTION_LAST_DOWN_TICK.store(0, AtomicOrdering::Relaxed);
            CAPTION_SKIP_UP.store(true, AtomicOrdering::Relaxed);
            toggle_caption_maximize(hwnd);
            return 0;
        } else if message == WM_NCLBUTTONDOWN {
            if wparam == HTCAPTION as usize {
                let raw_x = (lparam as i32) & 0xffff;
                let raw_y = ((lparam as u32) >> 16) as i32;
                let x = if raw_x >= 0x8000 { raw_x - 0x10000 } else { raw_x };
                let y = if raw_y >= 0x8000 { raw_y - 0x10000 } else { raw_y };
                // 手动双击兜底：系统未把第二次按下合成 DBLCLK 时（合成消息/异常路径），
                // 与上一次按下同点（双击矩形内）且在双击时间内 → 切换最大化/还原。
                // 位置复用 CAPTION_DOWN_*（存的是上一次按下，先比对再覆盖）。
                let now = GetTickCount();
                let last = CAPTION_LAST_DOWN_TICK.load(AtomicOrdering::Relaxed);
                let dbl_w = GetSystemMetrics(SM_CXDOUBLECLK);
                let dbl_h = GetSystemMetrics(SM_CYDOUBLECLK);
                if last != 0
                    && now.wrapping_sub(last) <= GetDoubleClickTime()
                    && (x - CAPTION_DOWN_X.load(AtomicOrdering::Relaxed)).abs() <= dbl_w
                    && (y - CAPTION_DOWN_Y.load(AtomicOrdering::Relaxed)).abs() <= dbl_h
                {
                    CAPTION_LAST_DOWN_TICK.store(0, AtomicOrdering::Relaxed);
                    CAPTION_SKIP_UP.store(true, AtomicOrdering::Relaxed);
                    toggle_caption_maximize(hwnd);
                    return 0;
                }
                // 阈值拖动第一步：吞掉按下（既不进 gpui 也不进 DefWindowProc），
                // 记录屏幕坐标并捕获鼠标，move/up 再决定是拖动还是点击
                CAPTION_LAST_DOWN_TICK.store(now, AtomicOrdering::Relaxed);
                CAPTION_DOWN_X.store(x, AtomicOrdering::Relaxed);
                CAPTION_DOWN_Y.store(y, AtomicOrdering::Relaxed);
                CAPTION_PENDING.store(true, AtomicOrdering::Relaxed);
                CAPTION_DRAGGING.store(false, AtomicOrdering::Relaxed);
                let _ = SetCapture(hwnd);
                return 0;
            }
        } else if CAPTION_PENDING.load(AtomicOrdering::Relaxed) {
            match message {
                WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
                    let mut cursor = POINT { x: 0, y: 0 };
                    let _ = GetCursorPos(&mut cursor);
                    if GetAsyncKeyState(VK_LBUTTON as i32) & 0x8000u16 as i16 == 0 {
                        // 抬起消息丢了（捕获异常）：按点击收尾，up 坐标取当前光标
                        CAPTION_PENDING.store(false, AtomicOrdering::Relaxed);
                        let _ = ReleaseCapture();
                        let mut up = cursor;
                        let _ = windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut up);
                        let up_lparam = ((up.y as u32) << 16 | (up.x as u32) & 0xffff) as LPARAM;
                        return finish_caption_click(hwnd, WM_LBUTTONUP, 0, up_lparam);
                    }
                    let scale = f32::from_bits(DRAG_SCALE.load(AtomicOrdering::Relaxed));
                    let threshold = (4.0 * scale) as i32;
                    let dx = cursor.x - CAPTION_DOWN_X.load(AtomicOrdering::Relaxed);
                    let dy = cursor.y - CAPTION_DOWN_Y.load(AtomicOrdering::Relaxed);
                    if dx * dx + dy * dy > threshold * threshold {
                        // 超阈值：启动系统移动循环（与真标题栏按下拖动一致）。
                        // DefWindowProc 直调不回流本 subclass；模态循环会泵消息，
                        // 置 OLE_MODAL_ACTIVE 挂起后台任务，避免 RefCell 重入（同 DoDragDrop 的坑）
                        CAPTION_PENDING.store(false, AtomicOrdering::Relaxed);
                        CAPTION_DRAGGING.store(true, AtomicOrdering::Relaxed);
                        CAPTION_LAST_DOWN_TICK.store(0, AtomicOrdering::Relaxed);
                        let _ = ReleaseCapture();
                        integration_log("caption drag begin");
                        let move_lparam =
                            ((cursor.y as u32) << 16 | (cursor.x as u32) & 0xffff) as LPARAM;
                        OLE_MODAL_ACTIVE.store(true, AtomicOrdering::Relaxed);
                        let result =
                            DefWindowProcW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, move_lparam);
                        OLE_MODAL_ACTIVE.store(false, AtomicOrdering::Relaxed);
                        CAPTION_DRAGGING.store(false, AtomicOrdering::Relaxed);
                        return result;
                    }
                    return 0;
                }
                WM_LBUTTONUP | WM_NCLBUTTONUP => {
                    CAPTION_PENDING.store(false, AtomicOrdering::Relaxed);
                    let _ = ReleaseCapture();
                    if CAPTION_DRAGGING.load(AtomicOrdering::Relaxed) {
                        CAPTION_DRAGGING.store(false, AtomicOrdering::Relaxed);
                        return 0;
                    }
                    return finish_caption_click(hwnd, message, wparam, lparam);
                }
                WM_CANCELMODE | WM_CAPTURECHANGED => {
                    // 失去捕获/进入模态：放弃这次点击，不做合成转发
                    CAPTION_PENDING.store(false, AtomicOrdering::Relaxed);
                    CAPTION_DRAGGING.store(false, AtomicOrdering::Relaxed);
                    return 0;
                }
                _ => {}
            }
        }
        if message == WM_FILEFLOW_TRAY && wparam == 0 && lparam as u32 == 0x10000 {
            // Win+E 钩子 PostMessage 到主窗口的唤起指令（托盘窗口收不到，这里兜底执行）
            focus_fileflow_window();
            return 0;
        }
        if CAPTION_SKIP_UP.load(AtomicOrdering::Relaxed)
            && (message == WM_LBUTTONUP || message == WM_NCLBUTTONUP)
        {
            // 双击标题带后的抬起：吞掉，避免孤立 MouseUp 进 gpui
            CAPTION_SKIP_UP.store(false, AtomicOrdering::Relaxed);
            return 0;
        }
        let original: WNDPROC = std::mem::transmute(GPUI_ORIG_WNDPROC.load(AtomicOrdering::Relaxed));
        CallWindowProcW(original, hwnd, message, wparam, lparam)
    }
}

/// 双击标题带：最大化/还原切换（与原生标题栏双击语义一致）。
/// 在 WndProc 内调用（此时 hwnd 就是目标窗口，不依赖前台窗口查询）。
/// 用 ShowWindowAsync 而非 ShowWindow：避免在 WndProc 内同步触发窗口过程重入。
unsafe fn toggle_caption_maximize(hwnd: HWND) {
    unsafe {
        let was_zoomed = IsZoomed(hwnd) != 0;
        let command = if was_zoomed { SW_RESTORE } else { SW_MAXIMIZE };
        integration_log(&format!(
            "caption dblclick: toggle maximize/restore was_zoomed={was_zoomed}"
        ));
        let ok = ShowWindowAsync(hwnd, command);
        integration_log(&format!(
            "caption dblclick: cmd={} ShowWindowAsync={ok}",
            if command == SW_MAXIMIZE { "SW_MAXIMIZE" } else { "SW_RESTORE" }
        ));
    }
}

/// 把缓存的标题带按下按"普通点击"转发给 gpui：合成客户区 WM_LBUTTONDOWN 再转发抬起。
/// 必须用客户区消息——gpui 未消费时 DefWindowProc(WM_LBUTTONDOWN) 是空操作，
/// 而 WM_NCLBUTTONDOWN+HTCAPTION 会直接启动系统拖动循环，把点击变成拖动。
unsafe fn finish_caption_click(
    hwnd: HWND,
    up_message: u32,
    up_wparam: WPARAM,
    up_lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let mut down = POINT {
            x: CAPTION_DOWN_X.load(AtomicOrdering::Relaxed),
            y: CAPTION_DOWN_Y.load(AtomicOrdering::Relaxed),
        };
        let _ = windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut down);
        let down_lparam = ((down.y as u32) << 16 | (down.x as u32) & 0xffff) as LPARAM;
        integration_log("caption click forwarded");
        let original: WNDPROC = std::mem::transmute(GPUI_ORIG_WNDPROC.load(AtomicOrdering::Relaxed));
        // 0x0001 = MK_LBUTTON（windows:: 的 MK_LBUTTON 是另一类型，这里用字面量）
        let _ = CallWindowProcW(original, hwnd, WM_LBUTTONDOWN, 0x0001, down_lparam);
        CallWindowProcW(original, hwnd, up_message, up_wparam, up_lparam)
    }
}
static GPUI_ORIG_WNDPROC: AtomicUsize = AtomicUsize::new(0);

/// 开窗后调用：subclass 顶层窗口接管 WM_NCHITTEST
fn install_drag_zone_subclass() {
    unsafe {
        let hwnd = cached_or_find_fileflow_hwnd();
        if hwnd.is_null() {
            integration_log("drag subclass: hwnd not found");
            return;
        }
        if GPUI_ORIG_WNDPROC.load(AtomicOrdering::Relaxed) != 0 {
            return; // 已安装
        }
        #[allow(unnecessary_transmutes, clippy::fn_to_numeric_cast, clippy::fn_to_numeric_cast_with_truncation, function_casts_as_integer)]
        let proc_ptr =
            std::mem::transmute::<usize, isize>(drag_zone_wndproc as usize);
        let original = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc_ptr);
        integration_log(&format!(
            "drag subclass: hwnd={:x} original={original:x}",
            hwnd as usize
        ));
        if original != 0 {
            let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd);
            let scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
            DRAG_SCALE.store(scale.to_bits(), AtomicOrdering::Relaxed);
            GPUI_ORIG_WNDPROC.store(original as usize, AtomicOrdering::Relaxed);
        }
    }
}

fn start_win_e_listener() {
    if !WIN_E_ENABLED.load(AtomicOrdering::Relaxed) {
        return;
    }
    if WIN_E_LISTENER_STARTED.swap(true, AtomicOrdering::Relaxed) {
        return;
    }
    thread::spawn(|| unsafe {
        let hwnd = ptr::null_mut();
        let mut message: MSG = std::mem::zeroed();
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(win_e_keyboard_hook),
            ptr::null_mut(),
            0,
        );
        if hook.is_null() {
            WIN_E_LISTENER_STARTED.store(false, AtomicOrdering::Relaxed);
            return;
        }
        // Low-level keyboard hooks are dispatched through this thread's message
        // queue. Sleeping here delays every system keystroke, including IME input.
        while GetMessageW(&mut message, hwnd, 0, 0) > 0 {
            let _ = TranslateMessage(&message);
            let _ = DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
        WIN_E_LISTENER_STARTED.store(false, AtomicOrdering::Relaxed);
    });
}

fn set_win_e_startup(enabled: bool) {
    if enabled {
        if let Ok(exe) = std::env::current_exe() {
            set_run_key("FileFlow", Some(&exe.display().to_string()));
        }
    } else {
        set_run_key("FileFlow", None);
    }
}

/// HKCU Run 启动项（Win32 API 直写，本机 reg.exe 被安全策略拦截不可用）
fn set_run_key(name: &str, value: Option<&str>) {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_WRITE, REG_SZ,
    };
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from_wide(&subkey[..subkey.len() - 1]),
            None,
            KEY_WRITE,
            &mut key,
        )
        .is_err()
        {
            return;
        }
        let name_wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        match value {
            Some(text) => {
                let mut data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
                let _ = RegSetValueExW(
                    key,
                    &HSTRING::from_wide(&name_wide[..name_wide.len() - 1]),
                    None,
                    REG_SZ,
                    Some(std::slice::from_raw_parts_mut(
                        data.as_mut_ptr() as *mut u8,
                        data.len() * 2,
                    )),
                );
            }
            None => {
                let _ = RegDeleteValueW(key, &HSTRING::from_wide(&name_wide[..name_wide.len() - 1]));
            }
        }
        let _ = RegCloseKey(key);
    }
}

/// 接管/恢复"文件夹默认打开方式"（Directory Opus 同款做法）：
/// HKCU\Software\Classes\{Directory,Drive}\shell 默认动词改为 FileFlow，
/// Affinity 等软件"导出后打开位置"即走 FileFlow。改前先备份到 registry-backup。
fn set_explorer_replacement(enabled: bool) -> Result<(), String> {
    let Ok(exe) = std::env::current_exe() else {
        return Err("无法获取程序路径".to_string());
    };
    let command = format!("\"{}\" \"%1\"", exe.display());
    for class in ["Directory", "Drive"] {
        let shell_key = format!("Software\\Classes\\{class}\\shell");
        if enabled {
            // 备份当前默认值（只备份一次，恢复时用）
            backup_shell_default(class, &shell_key);
            // 写 FileFlow 动词为默认
            write_reg_sz(&format!("{shell_key}\\FileFlow"), None, "用 FileFlow 打开");
            write_reg_sz(&format!("{shell_key}\\FileFlow\\command"), None, &command);
            write_reg_sz(&shell_key, None, "FileFlow");
        } else {
            // 恢复备份的默认值；没有备份则删除默认值回落系统默认
            let backup = read_backup_shell_default(class);
            match backup {
                Some(value) if !value.is_empty() => {
                    write_reg_sz(&shell_key, None, &value);
                }
                _ => {
                    delete_reg_value(&shell_key, None);
                }
            }
            delete_reg_tree(&format!("{shell_key}\\FileFlow"));
        }
    }
    notify_shell_changed();
    Ok(())
}

/// 通知 Shell 文件关联/默认值已变化（刷新图标与打开方式缓存）
fn notify_shell_changed() {
    use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null());
    }
}

fn registry_backup_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("registry-backup")
}

/// 备份 {class}\shell 的默认动词到 registry-backup/{class}.default.txt
fn backup_shell_default(class: &str, shell_key: &str) {
    let dir = registry_backup_dir();
    let _ = fs::create_dir_all(&dir);
    let backup_file = dir.join(format!("{class}.default.txt"));
    if backup_file.exists() {
        return; // 已有备份不覆盖（保留最初状态）
    }
    let current = read_reg_sz(shell_key, None).unwrap_or_default();
    let _ = fs::write(&backup_file, &current);
}

fn read_backup_shell_default(class: &str) -> Option<String> {
    fs::read_to_string(registry_backup_dir().join(format!("{class}.default.txt"))).ok()
}

/// 读 HKCU REG_SZ（子键 + 值名，None = 默认值）
fn read_reg_sz(subkey: &str, value: Option<&str>) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        REG_VALUE_TYPE, REG_SZ,
    };
    let mut key = HKEY::default();
    let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from_wide(&subkey_wide),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return None;
        }
        let name_hstring = value.map(HSTRING::from);
        let name = name_hstring
            .as_ref()
            .map(|hstring| windows::core::PCWSTR(hstring.as_ptr()))
            .unwrap_or_else(windows::core::PCWSTR::null);
        let mut size: u32 = 1024;
        let mut buffer = vec![0u8; size as usize];
        let result = RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut REG_VALUE_TYPE(REG_SZ.0)),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if result.is_err() {
            return None;
        }
        let units: Vec<u16> = buffer[..size as usize]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&unit| unit != 0)
            .collect();
        Some(String::from_utf16_lossy(&units))
    }
}

/// 写 HKCU REG_SZ（子键不存在则创建；值名 None = 默认值）
fn write_reg_sz(subkey: &str, value_name: Option<&str>, data: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from_wide(&subkey_wide),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .is_err()
        {
            return false;
        }
        let name_hstring = value_name.map(HSTRING::from);
        let name = name_hstring
            .as_ref()
            .map(|hstring| windows::core::PCWSTR(hstring.as_ptr()))
            .unwrap_or_else(windows::core::PCWSTR::null);
        let mut wide: Vec<u16> = data.encode_utf16().chain(Some(0)).collect();
        let bytes = std::slice::from_raw_parts_mut(wide.as_mut_ptr() as *mut u8, wide.len() * 2);
        let ok = RegSetValueExW(key, name, None, REG_SZ, Some(bytes)).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

/// 删除 HKCU 某子键下的值（值名 None = 默认值）
fn delete_reg_value(subkey: &str, value_name: Option<&str>) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    };
    let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from_wide(&subkey_wide),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let name_hstring = value_name.map(HSTRING::from);
        let name = name_hstring
            .as_ref()
            .map(|hstring| windows::core::PCWSTR(hstring.as_ptr()))
            .unwrap_or_else(windows::core::PCWSTR::null);
        let ok = RegDeleteValueW(key, name).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

/// 递归删除 HKCU 子键
fn delete_reg_tree(subkey: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteTreeW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
    };
    let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from_wide(&subkey_wide),
            None,
            KEY_WRITE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let ok = RegDeleteTreeW(key, None).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

fn compare_entries(a: &Entry, b: &Entry, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        SortColumn::Type => type_label(a)
            .cmp(&type_label(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        SortColumn::Modified => a
            .modified
            .cmp(&b.modified)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        SortColumn::Size => a
            .size
            .cmp(&b.size)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
    }
}

fn sort_column_label(column: SortColumn) -> &'static str {
    match column {
        SortColumn::Name => "名称",
        SortColumn::Type => "类型",
        SortColumn::Modified => "修改日期",
        SortColumn::Size => "大小",
    }
}

fn sort_header_label(label: &str, active: bool, ascending: bool) -> String {
    if active {
        format!("{label} {}", if ascending { "↑" } else { "↓" })
    } else {
        label.to_string()
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() > limit {
        format!("{}…", value.chars().take(limit).collect::<String>())
    } else {
        value.to_string()
    }
}
fn ellipsis_middle(value: &str, limit: usize) -> String {
    let count = value.chars().count();
    if count <= limit {
        return value.to_string();
    }
    if limit <= 2 {
        return "..".to_string();
    }
    let keep = limit.saturating_sub(2);
    format!("{}..", value.chars().take(keep).collect::<String>())
}
fn crumb_width_for(label: &str, compressed: bool) -> f32 {
    let units = label
        .chars()
        .map(|ch| if ch.is_ascii() { 9.5 } else { 19.0 })
        .sum::<f32>();
    let max_width = if compressed { 300.0 } else { 760.0 };
    (units + 34.0).clamp(46.0, max_width)
}
fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.)
    } else if bytes >= 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}
fn view_percent(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Details => "0%",
        ViewMode::List => "15%",
        ViewMode::Columns => "25%",
        ViewMode::MIcons => "35%",
        ViewMode::LIcons => "60%",
        ViewMode::XLIcons => "100%",
    }
}

struct ShortcutSpec {
    id: &'static str,
    label: &'static str,
    default_key: &'static str,
}

fn shortcut_specs() -> &'static [ShortcutSpec] {
    &[
        ShortcutSpec {
            id: "refresh",
            label: "刷新",
            default_key: "f5",
        },
        ShortcutSpec {
            id: "up",
            label: "返回上级",
            default_key: "alt-up",
        },
        ShortcutSpec {
            id: "back",
            label: "后退",
            default_key: "alt-left",
        },
        ShortcutSpec {
            id: "forward",
            label: "前进",
            default_key: "alt-right",
        },
        ShortcutSpec {
            id: "sidebar",
            label: "显示/隐藏侧边栏",
            default_key: "ctrl-b",
        },
        ShortcutSpec {
            id: "split",
            label: "切换双栏",
            default_key: "ctrl-shift-s",
        },
        ShortcutSpec {
            id: "new_tab",
            label: "新建标签页",
            default_key: "ctrl-t",
        },
        ShortcutSpec {
            id: "close_tab",
            label: "关闭标签页",
            default_key: "ctrl-w",
        },
        ShortcutSpec {
            id: "new_folder",
            label: "新建文件夹",
            default_key: "ctrl-n",
        },
        ShortcutSpec {
            id: "new_text_file",
            label: "新建 TXT 文件",
            default_key: "ctrl-shift-n",
        },
        ShortcutSpec {
            id: "hidden",
            label: "显示/隐藏隐藏项",
            default_key: "ctrl-h",
        },
        ShortcutSpec {
            id: "copy_other",
            label: "复制到另一栏",
            default_key: "ctrl-1",
        },
        ShortcutSpec {
            id: "move_other",
            label: "移动到另一栏",
            default_key: "ctrl-2",
        },
        ShortcutSpec {
            id: "copy_paths",
            label: "复制路径",
            default_key: "ctrl-shift-c",
        },
        ShortcutSpec {
            id: "select_all",
            label: "全选",
            default_key: "ctrl-a",
        },
        ShortcutSpec {
            id: "properties",
            label: "属性",
            default_key: "alt-a",
        },
        ShortcutSpec {
            id: "copy",
            label: "复制",
            default_key: "ctrl-c",
        },
        ShortcutSpec {
            id: "cut",
            label: "剪切",
            default_key: "ctrl-x",
        },
        ShortcutSpec {
            id: "paste",
            label: "粘贴",
            default_key: "ctrl-v",
        },
        ShortcutSpec {
            id: "filter",
            label: "筛选",
            default_key: "ctrl-f",
        },
        ShortcutSpec {
            id: "address",
            label: "定位到地址栏",
            default_key: "ctrl-e",
        },
        ShortcutSpec {
            id: "settings",
            label: "选项",
            default_key: "ctrl-comma",
        },
        ShortcutSpec {
            id: "shortcuts",
            label: "快捷键页面",
            default_key: "ctrl-alt-k",
        },
        ShortcutSpec {
            id: "preview",
            label: "预览",
            default_key: "space",
        },
        ShortcutSpec {
            id: "favorite",
            label: "收藏当前文件夹",
            default_key: "ctrl-d",
        },
        ShortcutSpec {
            id: "rename",
            label: "重命名",
            default_key: "f2",
        },
        ShortcutSpec {
            id: "delete",
            label: "删除到回收站",
            default_key: "delete",
        },
    ]
}

fn shortcut_key<'a>(bindings: &'a BTreeMap<String, String>, id: &str) -> &'a str {
    bindings
        .get(id)
        .map(String::as_str)
        .or_else(|| {
            shortcut_specs()
                .iter()
                .find(|spec| spec.id == id)
                .map(|spec| spec.default_key)
        })
        .expect("shortcut id must be declared")
}

fn parsed_shortcut_bindings(value: &serde_json::Value) -> BTreeMap<String, String> {
    let known: HashSet<_> = shortcut_specs().iter().map(|spec| spec.id).collect();
    let mut used = HashSet::new();
    value
        .get("shortcut_bindings")
        .cloned()
        .and_then(|value| serde_json::from_value::<BTreeMap<String, String>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|(id, key)| {
            known.contains(id.as_str())
                && Keystroke::parse(key).is_ok()
                && used.insert(key.to_ascii_lowercase())
        })
        .collect()
}

fn display_shortcut(key: &str) -> String {
    key.split('-')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "left" => "Left".to_string(),
            "right" => "Right".to_string(),
            "up" => "Up".to_string(),
            "down" => "Down".to_string(),
            other => {
                let mut chars = other.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn keystroke_binding(keystroke: &Keystroke) -> Option<String> {
    if keystroke.key.is_empty() || keystroke.key == "modifier" {
        return None;
    }
    let mut parts = Vec::with_capacity(4);
    if keystroke.modifiers.control {
        parts.push("ctrl".to_string());
    }
    if keystroke.modifiers.alt {
        parts.push("alt".to_string());
    }
    if keystroke.modifiers.shift {
        parts.push("shift".to_string());
    }
    if keystroke.modifiers.platform {
        parts.push("win".to_string());
    }
    parts.push(keystroke.key.to_ascii_lowercase());
    let binding = parts.join("-");
    Keystroke::parse(&binding).ok().map(|_| binding)
}

fn app_key_bindings(bindings: &BTreeMap<String, String>) -> Vec<KeyBinding> {
    vec![
        KeyBinding::new(shortcut_key(bindings, "refresh"), Refresh, None),
        KeyBinding::new(shortcut_key(bindings, "up"), GoUp, None),
        KeyBinding::new(shortcut_key(bindings, "back"), GoBack, None),
        KeyBinding::new(shortcut_key(bindings, "forward"), GoForward, None),
        KeyBinding::new(shortcut_key(bindings, "sidebar"), ToggleSidebar, None),
        KeyBinding::new(shortcut_key(bindings, "split"), ToggleSplit, None),
        KeyBinding::new(shortcut_key(bindings, "new_tab"), NewTab, None),
        KeyBinding::new(shortcut_key(bindings, "close_tab"), CloseTab, None),
        KeyBinding::new(shortcut_key(bindings, "new_folder"), NewFolder, None),
        KeyBinding::new(shortcut_key(bindings, "new_text_file"), NewTextFile, None),
        KeyBinding::new(shortcut_key(bindings, "hidden"), ToggleHidden, None),
        KeyBinding::new(shortcut_key(bindings, "copy_other"), CopyToOther, None),
        KeyBinding::new(shortcut_key(bindings, "move_other"), MoveToOther, None),
        KeyBinding::new(shortcut_key(bindings, "copy_paths"), CopyPaths, None),
        KeyBinding::new(shortcut_key(bindings, "select_all"), SelectAllEntries, None),
        KeyBinding::new(shortcut_key(bindings, "properties"), OpenProperties, None),
        KeyBinding::new(shortcut_key(bindings, "copy"), CopySelected, None),
        KeyBinding::new(shortcut_key(bindings, "cut"), CutSelected, None),
        KeyBinding::new(shortcut_key(bindings, "paste"), PasteFiles, None),
        KeyBinding::new(shortcut_key(bindings, "filter"), FocusFilter, None),
        KeyBinding::new(shortcut_key(bindings, "address"), FocusAddress, None),
        KeyBinding::new(shortcut_key(bindings, "settings"), OpenSettings, None),
        KeyBinding::new(shortcut_key(bindings, "shortcuts"), OpenShortcuts, None),
        KeyBinding::new(shortcut_key(bindings, "preview"), TogglePreview, None),
        KeyBinding::new(shortcut_key(bindings, "favorite"), ToggleFavorite, None),
        KeyBinding::new(shortcut_key(bindings, "rename"), RenameSelected, None),
        KeyBinding::new(shortcut_key(bindings, "delete"), DeleteSelected, None),
        // Shift+Del：永久删除（不进回收站、不可撤销）
        KeyBinding::new("shift-delete", PermanentDeleteSelected, None),
        KeyBinding::new("ctrl-z", Undo, None),
        KeyBinding::new("up", NavigateUp, None),
        KeyBinding::new("down", NavigateDown, None),
        KeyBinding::new("left", NavigateLeft, None),
        KeyBinding::new("right", NavigateRight, None),
        KeyBinding::new("ctrl-r", Refresh, None),
        KeyBinding::new("backspace", GoUp, None),
        KeyBinding::new("ctrl-l", FocusAddress, None),
        KeyBinding::new("ctrl-shift-right", NextView, None),
        KeyBinding::new("ctrl-shift-left", PreviousView, None),
        KeyBinding::new("ctrl-shift-f", CycleFolders, None),
        KeyBinding::new("escape", ClearTransient, None),
        KeyBinding::new("enter", SubmitAddress, None),
        KeyBinding::new("ctrl-shift-1", ViewDetails, None),
        KeyBinding::new("ctrl-shift-2", ViewList, None),
        KeyBinding::new("ctrl-shift-3", ViewColumns, None),
        KeyBinding::new("ctrl-shift-4", ViewMIcons, None),
        KeyBinding::new("ctrl-shift-5", ViewLIcons, None),
        KeyBinding::new("ctrl-shift-6", ViewXLIcons, None),
        KeyBinding::new("escape", ClearTransient, Some("FileFlowTextInput")),
        KeyBinding::new("enter", SubmitAddress, Some("FileFlowTextInput")),
        KeyBinding::new("shift-enter", text_input::InsertNewline, Some("FileFlowMultilineInput")),
        KeyBinding::new("ctrl-n", text_input::InsertNewline, Some("FileFlowMultilineInput")),
        KeyBinding::new("ctrl-shift-n", text_input::InsertNewline, Some("FileFlowMultilineInput")),
        KeyBinding::new("up", text_input::Up, Some("FileFlowMultilineInput")),
        KeyBinding::new("down", text_input::Down, Some("FileFlowMultilineInput")),
        KeyBinding::new("f5", Refresh, Some("FileFlowTextInput")),
        KeyBinding::new("space", text_input::InsertSpace, Some("FileFlowTextInput")),
        KeyBinding::new(
            "backspace",
            text_input::Backspace,
            Some("FileFlowTextInput"),
        ),
        KeyBinding::new("delete", text_input::Delete, Some("FileFlowTextInput")),
        KeyBinding::new("left", text_input::Left, Some("FileFlowTextInput")),
        KeyBinding::new("right", text_input::Right, Some("FileFlowTextInput")),
        KeyBinding::new(
            "shift-left",
            text_input::SelectLeft,
            Some("FileFlowTextInput"),
        ),
        KeyBinding::new(
            "shift-right",
            text_input::SelectRight,
            Some("FileFlowTextInput"),
        ),
        KeyBinding::new("ctrl-a", text_input::SelectAll, Some("FileFlowTextInput")),
        KeyBinding::new("ctrl-v", text_input::Paste, Some("FileFlowTextInput")),
        KeyBinding::new("ctrl-c", text_input::Copy, Some("FileFlowTextInput")),
        KeyBinding::new("ctrl-x", text_input::Cut, Some("FileFlowTextInput")),
        KeyBinding::new("home", text_input::Home, Some("FileFlowTextInput")),
        KeyBinding::new("end", text_input::End, Some("FileFlowTextInput")),
    ]
}

struct SavedConfig {
    split: bool,
    view_mode: ViewMode,
    left_path: PathBuf,
    right_path: PathBuf,
    show_hidden: bool,
    show_sidebar: bool,
    ui_font_size: f32,
    file_font_size: f32,
    row_spacing: f32,
    load_thumbnails: bool,
    win_e_enabled: bool,
    explorer_replacement: bool,
    favorites: Vec<PathBuf>,
    recents: Vec<PathBuf>,
    recent_limit: usize,
    shell_menu_default: bool,
    shell_menu_third_party: bool,
    shortcut_bindings: BTreeMap<String, String>,
}

fn config_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FileFlowEgui")
        .join("settings.json")
}

fn integration_log(message: &str) {
    let path = config_path().with_file_name("integration.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{} {message}",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        );
    }
}

fn json_bool(value: &serde_json::Value, key: &str, fallback: bool) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(fallback)
}

fn json_number(value: &serde_json::Value, key: &str, fallback: f32) -> f32 {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .map(|number| number as f32)
        .unwrap_or(fallback)
}

fn config_path_value(value: &serde_json::Value, key: &str, fallback: &Path) -> PathBuf {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| fallback.to_path_buf())
}

fn load_config(fallback: &Path) -> SavedConfig {
    let value = fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_default();
    SavedConfig {
        split: json_bool(&value, "split", false),
        view_mode: match value.get("view_mode").and_then(serde_json::Value::as_str) {
            Some("List") => ViewMode::List,
            Some("Columns") => ViewMode::Columns,
            Some("MIcons") => ViewMode::MIcons,
            Some("LIcons") => ViewMode::LIcons,
            Some("XLIcons") => ViewMode::XLIcons,
            _ => ViewMode::Details,
        },
        left_path: config_path_value(&value, "left_path", fallback),
        right_path: config_path_value(&value, "right_path", fallback),
        show_hidden: json_bool(&value, "show_hidden", false),
        show_sidebar: json_bool(&value, "show_sidebar", true),
        ui_font_size: json_number(&value, "ui_font_size", 14.).clamp(11., 24.),
        file_font_size: json_number(&value, "file_font_size", 14.).clamp(11., 26.),
        row_spacing: json_number(&value, "row_spacing", 6.).clamp(0., 24.),
        load_thumbnails: json_bool(&value, "load_thumbnails", true),
        win_e_enabled: json_bool(&value, "win_e_enabled", false),
        explorer_replacement: json_bool(&value, "explorer_replacement", false),
        favorites: value
            .get("favorites")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(PathBuf::from)
            .filter(|path| can_open_directory(path))
            .collect(),
        recent_limit: value
            .get("recent_limit")
            .and_then(serde_json::Value::as_u64)
            .map(|value| value.min(30) as usize)
            .unwrap_or(10),
        shell_menu_default: json_bool(&value, "shell_menu_default", true),
        shell_menu_third_party: json_bool(&value, "shell_menu_third_party", false),
        shortcut_bindings: parsed_shortcut_bindings(&value),
        recents: {
            let limit = value
                .get("recent_limit")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value.min(30) as usize)
                .unwrap_or(10);
            value
                .get("recents")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(PathBuf::from)
                .filter(|path| path.exists() || can_open_directory(path))
                .take(limit)
                .collect()
        },
    }
}

fn pixel_value(value: Pixels) -> f32 {
    value / px(1.)
}

/// 虚拟化网格布局几何（单一事实来源）：渲染行构建、拖框命中、拖放命中、
/// 键盘导航全部从这一个公式取参数，保证三者严丝合缝。
/// 返回 (cols, cell_w, cell_h, stride_x, stride_y, pad)；
/// rows() 按条目数算行数。宽度即滚动容器内容宽（px）。
pub(crate) struct VirtualGridGeometry {
    pub cols: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub stride_x: f32,
    pub stride_y: f32,
    pub pad: f32,
    pub count: usize,
}

impl VirtualGridGeometry {
    pub fn rows(&self) -> usize {
        if self.cols == 0 {
            return 0;
        }
        self.count.div_ceil(self.cols)
    }
    /// 第 index 个条目的 (left, top, right, bottom)，原点 = 容器内容区左上（未加滚动偏移）
    pub fn cell_rect(&self, index: usize) -> (f32, f32, f32, f32) {
        let col = index % self.cols;
        let row = index / self.cols;
        let left = self.pad + col as f32 * self.stride_x;
        let top = self.pad + row as f32 * self.stride_y;
        (left, top, left + self.cell_w, top + self.cell_h)
    }
}

/// view_mode 决定单元格尺寸；icon 视图参数与 entry_grid_cell 渲染严格一致：
/// cell_w = tile；cell_h = tile*0.74 + 3*label_line + 8（渲染侧 h(px(...))）；
/// gap = 8（渲染行 .gap_2()）；pad = 8（uniform_list .p_2()）。
/// columns 视图：cell_w=240, cell_h=column_row_h, gap=8（行内 .gap_2()）, pad=8。
pub(crate) fn virtual_grid_geometry(
    view_mode: ViewMode,
    font_size: f32,
    row_spacing: f32,
    pane_w: f32,
    count: usize,
) -> VirtualGridGeometry {
    let (cell_w, cell_h, gap, pad): (f32, f32, f32, f32) = match view_mode {
        ViewMode::MIcons | ViewMode::LIcons | ViewMode::XLIcons => {
            let tile = match view_mode {
                ViewMode::MIcons => 110.,
                ViewMode::LIcons => 160.,
                ViewMode::XLIcons => 220.,
                _ => 110.,
            };
            let label_line = ((font_size * 0.86).clamp(12., 18.) * 1.25).ceil();
            (tile, tile * 0.74 + 3.0 * label_line + 8., 8., 8.)
        }
        ViewMode::Columns => {
            let row_h = (font_size * 1.5 + row_spacing).max(28. + row_spacing);
            (240., row_h, 8., 8.)
        }
        _ => {
            // List/Details 不走网格几何；给个安全退化值
            let row_h = (font_size * 1.42 + row_spacing).max(27. + row_spacing);
            (pane_w.max(1.), row_h, 0., 0.)
        }
    };
    let stride_x = cell_w + gap;
    let stride_y = cell_h + gap;
    let cols = (((pane_w - pad * 2.0 + gap) / stride_x).floor() as usize).max(1);
    VirtualGridGeometry {
        cols,
        cell_w,
        cell_h,
        stride_x,
        stride_y,
        pad,
        count,
    }
}

fn rect_intersects(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    let (a_left, a_top, a_right, a_bottom) = a;
    let (b_left, b_top, b_right, b_bottom) = b;
    a_left <= b_right && a_right >= b_left && a_top <= b_bottom && a_bottom >= b_top
}

fn point_in_rect(point: (f32, f32), rect: (f32, f32, f32, f32)) -> bool {
    point.0 >= rect.0 && point.0 <= rect.2 && point.1 >= rect.1 && point.1 <= rect.3
}

unsafe extern "system" fn shell_menu_owner_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// 统一释放本进程分配的 PIDL。
/// 注意：SHBindToParent 返回的 child_pidl 只是 absolute_pidl 的内部指针
/// （等价 ILFindLastID），**绝不能**单独释放——释放堆块中间指针会堆损坏
/// （STATUS_HEAP_CORRUPTION 0xC0000374），表现为菜单关闭后 helper 静默崩溃。
fn free_pidls(pidls: &mut Vec<*mut ITEMIDLIST>) {
    for pidl in pidls.drain(..) {
        unsafe {
            CoTaskMemFree(Some(pidl as _));
        }
    }
}

/// STRRET（GetDisplayNameOf 返回）转 Rust String，统一走 Unicode 分支。
unsafe fn string_from_pwstr(strret: windows::Win32::UI::Shell::Common::STRRET) -> String {
    unsafe {
        let mut buffer = [0u16; 32768];
        let mut strret = strret;
        let ok = StrRetToBufW(&mut strret, None, &mut buffer);
        if ok.is_ok() {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            String::from_utf16_lossy(&buffer[..len])
        } else {
            String::new()
        }
    }
}

/// 枚举回调：跳过 owner 自身，发现任意可见顶层窗口即停止（返回 0）。
unsafe extern "system" fn visible_window_probe(hwnd: HWND, owner: LPARAM) -> i32 {
    if hwnd != owner as HWND && unsafe { IsWindowVisible(hwnd) } != 0 {
        return 0;
    }
    1
}

fn thread_has_visible_window_except(owner: HWND) -> bool {
    unsafe {
        // 回调提前停止（返回 0）时 EnumThreadWindows 返回 0，即“找到了”
        EnumThreadWindows(GetCurrentThreadId(), Some(visible_window_probe), owner as LPARAM) == 0
    }
}

/// Shell 动词执行后常把模式less对话框（如"属性"）建在本进程线程上，或把
/// 剪贴板 IDataObject 挂在本进程——进程一退就全没了。这里泵消息直到对话框
/// 关闭：先给 3 秒宽限等异步创建，之后只要本线程还有可见窗口就继续；
/// 30 分钟硬上限防僵尸。
fn pump_shell_command_windows(owner: HWND) {
    unsafe {
        let _ = ShowWindow(owner, SW_HIDE);
    }
    let start = std::time::Instant::now();
    loop {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let elapsed = start.elapsed().as_secs();
        if elapsed >= 3 && !thread_has_visible_window_except(owner) {
            return;
        }
        if elapsed >= 1800 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn create_shell_menu_owner() -> HWND {
    unsafe {
        let class_name = wide_null("FileFlowShellMenuOwner");
        let instance = GetModuleHandleW(ptr::null());
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(shell_menu_owner_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        let _ = RegisterClassW(&wnd_class);
        let mut cursor = POINT { x: 0, y: 0 };
        let _ = GetCursorPos(&mut cursor);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            wide_null("FileFlow Shell Menu").as_ptr(),
            WS_POPUP,
            cursor.x,
            cursor.y,
            1,
            1,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }
        integration_log(&format!("shell menu owner hwnd={hwnd:p}"));
        hwnd
    }
}

fn show_windows_shell_context_menu_with_owner(
    request: &ShellMenuRequest,
    owner: WHWND,
) -> Result<(), String> {
    let Some(primary) = request.paths.first() else {
        return Err("没有可用的右键目标".to_string());
    };
    integration_log(&format!(
        "shell menu begin primary={} count={} blank={} third_party={}",
        primary.display(),
        request.paths.len(),
        request.blank,
        request.include_third_party
    ));
    let mut primary_wide: Vec<u16> = primary.display().to_string().encode_utf16().collect();
    primary_wide.push(0);
    unsafe {
        // OleInitialize 而非 CoInitializeEx：OleFlushClipboard 与 Shell 剪贴板
        // 数据对象要求 OLE 已初始化
        let ole_result = OleInitialize(None);
        let should_uninitialize = ole_result.is_ok();
        if let Err(error) = &ole_result
            && error.code() != RPC_E_CHANGED_MODE
        {
            return Err(format!("COM 初始化失败：{error}"));
        }
        // 统一在闭包内完成构建菜单与弹出，错误路径统一走 drop 清理
        let outcome = (|| -> Result<bool, String> {
            let mut absolute_pidl: *mut ITEMIDLIST = ptr::null_mut();
            let parse_result = SHParseDisplayName(
                WPCWSTR(primary_wide.as_ptr()),
                None::<&IBindCtx>,
                &mut absolute_pidl,
                0,
                None,
            );
            if let Err(error) = parse_result {
                return Err(format!("解析路径失败：{error}"));
            }
            // 需要最后统一释放的 PIDL。SHBindToParent 给的 child_pidl 只是
            // absolute_pidl 的内部指针（等价 ILFindLastID），不进这个表、绝不单独释放。
            let mut owned_pidls: Vec<*mut ITEMIDLIST> = vec![absolute_pidl];

            let mut child_pidl: *mut ITEMIDLIST = ptr::null_mut();
            let parent_folder: IShellFolder =
                match SHBindToParent(absolute_pidl, Some(&mut child_pidl)) {
                    Ok(folder) => folder,
                    Err(error) => {
                        free_pidls(&mut owned_pidls);
                        return Err(format!("绑定 Shell 文件夹失败：{error}"));
                    }
                };

            // 多选：为每个选中项获取父文件夹内的相对 PIDL；单选直接用 child_pidl
            let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::new();
            if request.paths.len() > 1 {
                for path in &request.paths {
                    let text = path.display().to_string();
                    let mut wide: Vec<u16> = text.encode_utf16().collect();
                    wide.push(0);
                    let mut pidl: *mut ITEMIDLIST = ptr::null_mut();
                    let parsed = parent_folder.ParseDisplayName(
                        WHWND::default(),
                        None::<&IBindCtx>,
                        WPCWSTR(wide.as_mut_ptr()),
                        None,
                        &mut pidl,
                        ptr::null_mut(),
                    );
                    if parsed.is_ok() && !pidl.is_null() {
                        child_pidls.push(pidl);
                        owned_pidls.push(pidl);
                    } else if !pidl.is_null() {
                        CoTaskMemFree(Some(pidl as _));
                    }
                }
            } else {
                child_pidls.push(child_pidl);
            }
            if child_pidls.is_empty() {
                drop(parent_folder);
                free_pidls(&mut owned_pidls);
                return Err("无法获取选中项的 Shell 标识".to_string());
            }

            let menu_result: windows_core::Result<IContextMenu> = if request.blank {
                // 空白菜单必须来自目录自身的 IShellFolder（此前误用父目录，
                // 粘贴/新建/属性全作用到上一级目录）
                let self_folder: windows_core::Result<IShellFolder> =
                    SHBindToObject(&parent_folder, child_pidl, None::<&IBindCtx>);
                match self_folder {
                    Ok(folder) => folder.CreateViewObject::<IContextMenu>(owner),
                    Err(error) => Err(error),
                }
            } else {
                parent_folder.GetUIObjectOf(owner, &child_pidls, None)
            };
            let menu: IContextMenu = match menu_result {
                Ok(menu) => menu,
                Err(error) => {
                    drop(parent_folder);
                    free_pidls(&mut owned_pidls);
                    return Err(format!("获取 Shell 菜单失败：{error}"));
                }
            };

            let popup = match WCreatePopupMenu() {
                Ok(menu) => menu,
                Err(error) => {
                    drop(menu);
                    drop(parent_folder);
                    free_pidls(&mut owned_pidls);
                    return Err(format!("创建菜单失败：{error}"));
                }
            };

            let menu_flags = CMF_NORMAL
                | if request.include_third_party {
                    CMF_EXTENDEDVERBS
                } else {
                    0
                };
            let query = menu.QueryContextMenu(popup, 0, 1, 0x7fff, menu_flags);
            if query.is_err() {
                let _ = WDestroyMenu(popup);
                for pidl in child_pidls {
                    CoTaskMemFree(Some(pidl as _));
                }
                return Err(format!("生成菜单失败 0x{:08X}", query.0 as u32));
            }

            // 注入 FileFlow 自定义菜单项（顶部），Shell 原生项保留在下方
            let injected = inject_fileflow_items(popup, request);
            if let Err(error) = injected {
                integration_log(&format!("注入自定义菜单失败：{error}"));
            }

            let mut point = WPOINT::default();
            WGetCursorPos(&mut point).map_err(|error| format!("读取鼠标位置失败：{error}"))?;
            let command = WTrackPopupMenu(
                popup,
                WTPM_RETURNCMD | WTPM_RIGHTBUTTON | WTPM_LEFTALIGN | WTPM_TOPALIGN,
                point.x,
                point.y,
                Some(0),
                owner,
                None,
            );
            integration_log(&format!(
                "shell menu TrackPopupMenu command={}",
                command.0
            ));
            let command_id = command.0 as u32;
            let is_custom = command.0 > 0
                && (MenuCommand::from_id(command_id).is_some()
                    || MenuCommand::view_token_from_id(command_id).is_some()
                    || MenuCommand::sort_token_from_id(command_id).is_some());
            if command.0 > 0 && !is_custom {
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: owner,
                    lpVerb: WPCSTR((command.0 as usize - 1) as *const u8),
                    nShow: WSW_SHOWNORMAL.0,
                    ..Default::default()
                };
                if let Err(error) = menu.InvokeCommand(&info) {
                    drop(menu);
                    drop(parent_folder);
                    let _ = WDestroyMenu(popup);
                    free_pidls(&mut owned_pidls);
                    return Err(format!("执行菜单命令失败：{error}"));
                }
                // "复制/剪切"由 Shell 把 IDataObject 挂在本进程剪贴板上，
                // 固化成内存数据，否则 helper 退出后剪贴板变空、粘贴无效。
                let _ = OleFlushClipboard();
                // "属性"等动词把对话框建在本线程上，泵消息直到对话框关闭再退出
                pump_shell_command_windows(owner.0);
            }

            // 先放 COM 对象（可能持有 PIDL 引用），再释放本进程分配的 PIDL
            drop(menu);
            drop(parent_folder);
            let _ = WDestroyMenu(popup);
            free_pidls(&mut owned_pidls);
            if command.0 > 0 {
                if let Some(menu_command) = MenuCommand::from_id(command_id) {
                    send_menu_command_to_main(menu_command, request.side);
                } else if let Some(token) = MenuCommand::view_token_from_id(command_id)
                    .or_else(|| MenuCommand::sort_token_from_id(command_id))
                {
                    send_menu_token_to_main(token, request.side);
                }
            }
            Ok(is_custom)
        })();

        if should_uninitialize {
            OleUninitialize();
        }
        outcome?;
    }
    integration_log("shell menu end");
    Ok(())
}

fn menu_item_string(text: &str) -> Vec<u16> {
    let mut value: Vec<u16> = text.encode_utf16().collect();
    value.push(0);
    value
}

unsafe fn insert_menu_item(
    popup: windows::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
    id: u32,
    label: &str,
) -> bool {
    let mut text = menu_item_string(label);
    let info = windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW {
        cbSize: std::mem::size_of::<
            windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW,
        >() as u32,
        fMask: windows::Win32::UI::WindowsAndMessaging::MIIM_ID
            | windows::Win32::UI::WindowsAndMessaging::MIIM_STRING,
        wID: id,
        dwTypeData: windows_core::PWSTR(text.as_mut_ptr()),
        cch: label.encode_utf16().count() as u32,
        ..Default::default()
    };
    unsafe { windows::Win32::UI::WindowsAndMessaging::InsertMenuItemW(popup, position, true, &info) }
        .is_ok()
}

unsafe fn insert_menu_separator(
    popup: windows::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
) -> bool {
    let info = windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW {
        cbSize: std::mem::size_of::<
            windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW,
        >() as u32,
        fMask: windows::Win32::UI::WindowsAndMessaging::MIIM_FTYPE,
        fType: windows::Win32::UI::WindowsAndMessaging::MFT_SEPARATOR,
        ..Default::default()
    };
    unsafe { windows::Win32::UI::WindowsAndMessaging::InsertMenuItemW(popup, position, true, &info) }
        .is_ok()
}

unsafe fn insert_menu_popup(
    popup: windows::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
    id: u32,
    label: &str,
    submenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
) -> bool {
    let mut text = menu_item_string(label);
    let info = windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW {
        cbSize: std::mem::size_of::<
            windows::Win32::UI::WindowsAndMessaging::MENUITEMINFOW,
        >() as u32,
        fMask: windows::Win32::UI::WindowsAndMessaging::MIIM_SUBMENU
            | windows::Win32::UI::WindowsAndMessaging::MIIM_STRING
            | windows::Win32::UI::WindowsAndMessaging::MIIM_ID,
        wID: id,
        hSubMenu: submenu,
        dwTypeData: windows_core::PWSTR(text.as_mut_ptr()),
        cch: label.encode_utf16().count() as u32,
        ..Default::default()
    };
    unsafe { windows::Win32::UI::WindowsAndMessaging::InsertMenuItemW(popup, position, true, &info) }
        .is_ok()
}

/// 在 Shell 原生 HMENU 顶部插入 FileFlow 自定义项。
/// 返回插入的自定义命令 id 数（不含纯导航子菜单项）。
unsafe fn inject_fileflow_items(
    popup: windows::Win32::UI::WindowsAndMessaging::HMENU,
    request: &ShellMenuRequest,
) -> Result<usize, String> {
    unsafe {
    let mut position: u32 = 0;
    let mut count = 0usize;
    let separator_id: u32 = MENU_CMD_BASE + 0x600;

    if request.blank {
        // 空白处：查看 / 排序方式 真子菜单 + 常用文件夹操作
        let view_submenu = windows::Win32::UI::WindowsAndMessaging::CreatePopupMenu()
            .map_err(|error| format!("{error}"))?;
        for (index, mode) in [
            ViewMode::Details,
            ViewMode::List,
            ViewMode::Columns,
            ViewMode::MIcons,
            ViewMode::LIcons,
            ViewMode::XLIcons,
        ]
        .into_iter()
        .enumerate()
        {
            insert_menu_item(view_submenu, index as u32, MENU_CMD_BASE + 0x100 + index as u32, mode.label());
        }
        let sort_submenu = windows::Win32::UI::WindowsAndMessaging::CreatePopupMenu()
            .map_err(|error| format!("{error}"))?;
        for (index, label) in ["名称", "类型", "修改日期", "大小"]
            .into_iter()
            .enumerate()
        {
            insert_menu_item(
                sort_submenu,
                index as u32,
                MENU_CMD_BASE + 0x200 + index as u32,
                label,
            );
        }
        if !insert_menu_popup(popup, position, separator_id, "查看(V)", view_submenu) {
            return Err("插入查看子菜单失败".to_string());
        }
        position += 1;
        if !insert_menu_popup(popup, position, separator_id + 1, "排序方式(O)", sort_submenu) {
            return Err("插入排序子菜单失败".to_string());
        }
        position += 1;
        insert_menu_separator(popup, position);
        position += 1;
        let commands = [
            (MenuCommand::NewFolder, true),
            (MenuCommand::NewText, true),
            (MenuCommand::Paste, true),
            (MenuCommand::Terminal, true),
            (MenuCommand::Refresh, true),
        ];
        for (command, enabled) in commands {
            if !enabled {
                continue;
            }
            if !insert_menu_item(popup, position, command.id(), command.label()) {
                return Err(format!("插入 {:?} 失败", command.token()));
            }
            position += 1;
            count += 1;
        }
        insert_menu_separator(popup, position);
        // 空白处不注入选择相关命令；属性由 Shell 背景菜单原生提供
    } else {
        let is_dir = request.paths.iter().all(|path| path.is_dir());
        let single = request.paths.len() == 1;
        // 文件项：打开 + 传送 + 剪贴板操作
        let mut commands: Vec<(MenuCommand, bool)> = vec![
            (MenuCommand::Open, true),
            (MenuCommand::OpenInOther, is_dir),
        ];
        if single || !is_dir {
            commands.extend([
                (MenuCommand::Copy, true),
                (MenuCommand::Cut, true),
                (MenuCommand::Paste, true),
                // 多选也可重命名：进入批量重命名模式
                (MenuCommand::Rename, true),
                (MenuCommand::Delete, true),
            ]);
        } else {
            commands.extend([
                (MenuCommand::Copy, true),
                (MenuCommand::Cut, true),
                (MenuCommand::Rename, true),
                (MenuCommand::Delete, true),
            ]);
        }
        commands.extend([
            (MenuCommand::CopyPath, true),
            (MenuCommand::Favorite, is_dir),
            (MenuCommand::OpenExplorer, true),
            (MenuCommand::Reveal, true),
            (MenuCommand::Properties, true),
            (MenuCommand::Terminal, is_dir),
            (MenuCommand::Refresh, true),
        ]);
        for (command, enabled) in commands {
            if !enabled {
                continue;
            }
            let label = if command == MenuCommand::Favorite && is_dir {
                "收藏文件夹"
            } else if command == MenuCommand::Rename && !single {
                "批量重命名\tF2"
            } else {
                command.label()
            };
            if !insert_menu_item(popup, position, command.id(), label) {
                return Err(format!("插入 {:?} 失败", command.token()));
            }
            position += 1;
            count += 1;
        }
        insert_menu_separator(popup, position);
    }
    Ok(count)
    }
}

#[implement(IDataObject)]
struct WindowsFileDragData {
    files: Vec<PathBuf>,
}

#[allow(non_snake_case)]
impl IDataObject_Impl for WindowsFileDragData_Impl {
    fn GetData(&self, format: *const FORMATETC) -> WResult<STGMEDIUM> {
        if !supports_hdrop(format) {
            return Err(WError::from_hresult(DV_E_FORMATETC));
        }
        let hglobal = build_hdrop_global(&self.files)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> WResult<()> {
        Err(WError::from_hresult(E_NOTIMPL))
    }

    fn QueryGetData(&self, format: *const FORMATETC) -> windows::core::HRESULT {
        if supports_hdrop(format) {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _: *const FORMATETC,
        _: *mut FORMATETC,
    ) -> windows::core::HRESULT {
        E_NOTIMPL
    }

    fn SetData(
        &self,
        _: *const FORMATETC,
        _: *const STGMEDIUM,
        _: windows::core::BOOL,
    ) -> WResult<()> {
        Err(WError::from_hresult(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, direction: u32) -> WResult<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(WError::from_hresult(E_NOTIMPL));
        }
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        unsafe { SHCreateStdEnumFmtEtc(&[format]) }
    }

    fn DAdvise(
        &self,
        _: *const FORMATETC,
        _: u32,
        _: windows::core::Ref<'_, IAdviseSink>,
    ) -> WResult<u32> {
        Err(WError::from_hresult(E_NOTIMPL))
    }

    fn DUnadvise(&self, _: u32) -> WResult<()> {
        Err(WError::from_hresult(E_NOTIMPL))
    }

    fn EnumDAdvise(&self) -> WResult<IEnumSTATDATA> {
        Err(WError::from_hresult(E_NOTIMPL))
    }
}

#[implement(IDropSource)]
struct WindowsFileDropSource;

#[allow(non_snake_case)]
impl IDropSource_Impl for WindowsFileDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        escape_pressed: windows::core::BOOL,
        key_state: MODIFIERKEYS_FLAGS,
    ) -> windows::core::HRESULT {
        if escape_pressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        if key_state.0 & MK_LBUTTON.0 == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _: DROPEFFECT) -> windows::core::HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

fn supports_hdrop(format: *const FORMATETC) -> bool {
    if format.is_null() {
        return false;
    }
    let format = unsafe { &*format };
    format.cfFormat == CF_HDROP.0
        && format.dwAspect == DVASPECT_CONTENT.0
        && (format.tymed & TYMED_HGLOBAL.0 as u32) != 0
}

fn build_hdrop_global(files: &[PathBuf]) -> WResult<windows::Win32::Foundation::HGLOBAL> {
    let wide_paths: Vec<Vec<u16>> = files
        .iter()
        .filter(|path| !is_virtual_path(path))
        .map(|path| path.as_os_str().encode_wide().chain(Some(0)).collect())
        .collect();
    if wide_paths.is_empty() {
        return Err(WError::from_hresult(DV_E_FORMATETC));
    }

    let header_size = std::mem::size_of::<DROPFILES>();
    let char_count: usize = wide_paths.iter().map(Vec::len).sum::<usize>() + 1;
    let byte_len = header_size + char_count * std::mem::size_of::<u16>();
    unsafe {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, byte_len)?;
        let data = GlobalLock(hglobal);
        if data.is_null() {
            return Err(WError::from_win32());
        }
        (data as *mut DROPFILES).write(DROPFILES {
            pFiles: header_size as u32,
            pt: WPOINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        });
        let mut cursor = (data as *mut u8).add(header_size) as *mut u16;
        for wide_path in wide_paths {
            ptr::copy_nonoverlapping(wide_path.as_ptr(), cursor, wide_path.len());
            cursor = cursor.add(wide_path.len());
        }
        cursor.write(0);
        let _ = GlobalUnlock(hglobal);
        Ok(hglobal)
    }
}

fn put_files_on_windows_clipboard(files: &[PathBuf], effect: DROPEFFECT) -> Result<(), String> {
    let files = files
        .iter()
        .filter(|path| !is_virtual_path(path))
        .cloned()
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Err("没有可复制的本地文件".to_string());
    }
    match put_shell_files_on_clipboard(&files, effect) {
        Ok(()) => Ok(()),
        Err(shell_error) => put_hdrop_files_on_clipboard(&files, effect)
            .map_err(|fallback_error| format!("{shell_error}；兼容模式也失败：{fallback_error}")),
    }
}

fn put_shell_files_on_clipboard(files: &[PathBuf], effect: DROPEFFECT) -> Result<(), String> {
    let Some(parent) = files.first().and_then(|path| path.parent()) else {
        return Err("无法确定文件所在目录".to_string());
    };
    if files.iter().any(|path| path.parent() != Some(parent)) {
        return Err("所选文件不在同一目录".to_string());
    }
    unsafe {
        let co_result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let should_uninitialize = co_result == S_OK;
        if co_result.is_err() && co_result != RPC_E_CHANGED_MODE {
            return Err(format!("COM 初始化失败：{co_result:?}"));
        }

        let mut allocated_pidls: Vec<*mut ITEMIDLIST> = Vec::new();
        let result = (|| -> Result<(), String> {
            let mut parent_wide: Vec<u16> =
                parent.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut parent_pidl = ptr::null_mut();
            SHParseDisplayName(
                WPCWSTR(parent_wide.as_mut_ptr()),
                None::<&IBindCtx>,
                &mut parent_pidl,
                0,
                None,
            )
            .map_err(|error| format!("Shell 解析目录失败：{error}"))?;
            allocated_pidls.push(parent_pidl);

            let mut child_pidls = Vec::with_capacity(files.len());
            for path in files {
                let mut path_wide: Vec<u16> =
                    path.as_os_str().encode_wide().chain(Some(0)).collect();
                let mut absolute_pidl = ptr::null_mut();
                SHParseDisplayName(
                    WPCWSTR(path_wide.as_mut_ptr()),
                    None::<&IBindCtx>,
                    &mut absolute_pidl,
                    0,
                    None,
                )
                .map_err(|error| format!("Shell 解析文件失败：{error}"))?;
                allocated_pidls.push(absolute_pidl);
                child_pidls.push(ILFindLastID(absolute_pidl) as *const ITEMIDLIST);
            }

            let data: IDataObject = SHCreateDataObject(
                Some(parent_pidl as *const ITEMIDLIST),
                Some(&child_pidls),
                None::<&IDataObject>,
            )
            .map_err(|error| format!("Shell 创建剪贴板对象失败：{error}"))?;

            let format_name: Vec<u16> = "Preferred DropEffect"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let preferred_format = RegisterClipboardFormatW(WPCWSTR(format_name.as_ptr()));
            if preferred_format != 0 {
                let effect_memory = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>())
                    .map_err(|error| format!("分配剪贴板内存失败：{error}"))?;
                let effect_ptr = GlobalLock(effect_memory) as *mut u32;
                if effect_ptr.is_null() {
                    let _ = GlobalFree(Some(effect_memory));
                    return Err("锁定剪贴板内存失败".to_string());
                }
                effect_ptr.write(effect.0);
                let _ = GlobalUnlock(effect_memory);
                let format = FORMATETC {
                    cfFormat: preferred_format as u16,
                    ptd: ptr::null_mut(),
                    dwAspect: DVASPECT_CONTENT.0,
                    lindex: -1,
                    tymed: TYMED_HGLOBAL.0 as u32,
                };
                let medium = STGMEDIUM {
                    tymed: TYMED_HGLOBAL.0 as u32,
                    u: STGMEDIUM_0 {
                        hGlobal: effect_memory,
                    },
                    pUnkForRelease: ManuallyDrop::new(None),
                };
                if let Err(error) = data.SetData(&format, &medium, true) {
                    let _ = GlobalFree(Some(effect_memory));
                    return Err(format!("设置复制效果失败：{error}"));
                }
            }

            OleSetClipboard(&data).map_err(|error| format!("写入 Shell 剪贴板失败：{error}"))?;
            OleFlushClipboard().map_err(|error| format!("持久化 Shell 剪贴板失败：{error}"))?;
            Ok(())
        })();

        for pidl in allocated_pidls {
            CoTaskMemFree(Some(pidl as _));
        }
        if should_uninitialize {
            CoUninitialize();
        }
        result
    }
}

fn put_hdrop_files_on_clipboard(files: &[PathBuf], effect: DROPEFFECT) -> Result<(), String> {
    let hglobal = build_hdrop_global(files).map_err(|error| error.to_string())?;
    let hwnd = cached_or_find_fileflow_hwnd();
    if hwnd.is_null() {
        unsafe {
            let _ = GlobalFree(Some(hglobal));
        }
        return Err("找不到 FileFlow 窗口".to_string());
    }
    unsafe {
        let mut opened = false;
        let mut open_error = None;
        for _ in 0..10 {
            match OpenClipboard(Some(WHWND(hwnd))) {
                Ok(()) => {
                    opened = true;
                    break;
                }
                Err(error) => {
                    open_error = Some(error);
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        if !opened {
            let _ = GlobalFree(Some(hglobal));
            return Err(format!(
                "剪贴板正被占用：{}",
                open_error
                    .map(|error| error.to_string())
                    .unwrap_or_default()
            ));
        }
        let result = (|| -> WResult<()> {
            EmptyClipboard()?;
            SetClipboardData(CF_HDROP.0 as u32, Some(WHANDLE(hglobal.0)))?;

            // Explorer also publishes this format so paste targets know this is a copy operation.
            let format_name: Vec<u16> = "Preferred DropEffect"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let preferred_format = RegisterClipboardFormatW(WPCWSTR(format_name.as_ptr()));
            if preferred_format != 0
                && let Ok(effect_memory) = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>())
            {
                let effect_ptr = GlobalLock(effect_memory) as *mut u32;
                if !effect_ptr.is_null() {
                    effect_ptr.write(effect.0);
                    let _ = GlobalUnlock(effect_memory);
                    if SetClipboardData(preferred_format, Some(WHANDLE(effect_memory.0))).is_err() {
                        let _ = GlobalFree(Some(effect_memory));
                    }
                } else {
                    let _ = GlobalFree(Some(effect_memory));
                }
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        if let Err(error) = result {
            // Ownership transfers only after a successful SetClipboardData call.
            let _ = GlobalFree(Some(hglobal));
            return Err(error.to_string());
        }
        Ok(())
    }
}

fn files_from_windows_clipboard() -> Result<(Vec<PathBuf>, bool), String> {
    unsafe {
        OpenClipboard(None).map_err(|error| error.to_string())?;
        let result = (|| -> WResult<(Vec<PathBuf>, bool)> {
            let handle = GetClipboardData(CF_HDROP.0 as u32)?;
            let hdrop = HDROP(handle.0);
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            let mut paths = Vec::with_capacity(count as usize);
            for index in 0..count {
                let length = DragQueryFileW(hdrop, index, None) as usize;
                let mut buffer = vec![0u16; length + 1];
                let written = DragQueryFileW(hdrop, index, Some(&mut buffer)) as usize;
                paths.push(PathBuf::from(std::ffi::OsString::from_wide(
                    &buffer[..written],
                )));
            }
            let format_name: Vec<u16> = "Preferred DropEffect"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let preferred_format = RegisterClipboardFormatW(WPCWSTR(format_name.as_ptr()));
            let move_items = if preferred_format != 0 {
                GetClipboardData(preferred_format)
                    .ok()
                    .and_then(|handle| {
                        let memory = windows::Win32::Foundation::HGLOBAL(handle.0);
                        let pointer = GlobalLock(memory) as *const u32;
                        if pointer.is_null() {
                            None
                        } else {
                            let effect = pointer.read();
                            let _ = GlobalUnlock(memory);
                            Some(effect & DROPEFFECT_MOVE.0 != 0)
                        }
                    })
                    .unwrap_or(false)
            } else {
                false
            };
            Ok((paths, move_items))
        })()
        .map_err(|error| error.to_string());
        let _ = CloseClipboard();
        result
    }
}

/// 拖拽放回 UI 线程执行（跨线程 DoDragDrop 收不到鼠标释放，永不返回）。
/// RefCell 崩溃改为用 OLE_MODAL_ACTIVE 标志防护：模态循环期间后台任务
/// 暂缓 view.update，循环退出后恢复。
fn start_windows_file_drag(files: &[PathBuf]) -> bool {
    let files: Vec<PathBuf> = files
        .iter()
        .filter(|path| !is_virtual_path(path))
        .cloned()
        .map(normalize_path)
        .collect();
    if files.is_empty() {
        return false;
    }
    integration_log(&format!("drag begin files={}", files.len()));
    OLE_MODAL_ACTIVE.store(true, AtomicOrdering::Relaxed);
    let ok = unsafe {
        let ole_result = OleInitialize(None);
        if ole_result.is_err() {
            integration_log(&format!("drag OleInitialize failed={ole_result:?}"));
            false
        } else {
            // CF_HDROP is Windows' native file-list format. Publishing it directly avoids
            // loading third-party shell extensions during a drag.
            let data: IDataObject = WindowsFileDragData { files }.into();
            let source: IDropSource = WindowsFileDropSource.into();
            let mut effect = DROPEFFECT(0);
            let allowed_effects =
                DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_MOVE.0 | DROPEFFECT_LINK.0);
            let result = DoDragDrop(&data, &source, allowed_effects, &mut effect);
            integration_log(&format!(
                "drag end hdrop_data=true result=0x{:08X} effect=0x{:X}",
                result.0 as u32, effect.0
            ));
            result == S_OK || result == DRAGDROP_S_DROP
        }
    };
    OLE_MODAL_ACTIVE.store(false, AtomicOrdering::Relaxed);
    ok
}

struct ShellMenuRequest {
    /// 空 menu 表示空白处右键，目标是当前文件夹自身
    blank: bool,
    /// 左右栏标识，回传命令时带上
    side: &'static str,
    /// 右键命中的文件；空白处右键时为当前文件夹
    paths: Vec<PathBuf>,
    include_third_party: bool,
}

fn parse_shell_menu_args(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Option<ShellMenuRequest> {
    let mut request = ShellMenuRequest {
        blank: false,
        side: "left",
        paths: Vec::new(),
        include_third_party: false,
    };
    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy().to_string();
        match text.as_str() {
            "--blank" => request.blank = true,
            "--left" => request.side = "left",
            "--right" => request.side = "right",
            "--third-party" => request.include_third_party = true,
            "--paths" => {
                while let Some(path) = iter.peek() {
                    let candidate = path.to_string_lossy().to_string();
                    if candidate.starts_with("--") {
                        break;
                    }
                    iter.next();
                    if let Some(path) = clean_external_path_arg(&candidate) {
                        request.paths.push(path);
                    }
                }
            }
            _ => {}
        }
    }
    if request.paths.is_empty() {
        None
    } else {
        Some(request)
    }
}

const MENU_CMD_BASE: u32 = 0x9000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuCommand {
    Open,
    OpenInOther,
    Copy,
    Cut,
    Paste,
    Rename,
    Delete,
    NewFolder,
    NewText,
    CopyPath,
    Properties,
    Favorite,
    OpenExplorer,
    Reveal,
    Terminal,
    Refresh,
}

impl MenuCommand {
    fn id(self) -> u32 {
        MENU_CMD_BASE + (self as u32)
    }
    fn from_id(id: u32) -> Option<Self> {
        if id < MENU_CMD_BASE {
            return None;
        }
        Some(match id - MENU_CMD_BASE {
            0 => Self::Open,
            1 => Self::OpenInOther,
            2 => Self::Copy,
            3 => Self::Cut,
            4 => Self::Paste,
            5 => Self::Rename,
            6 => Self::Delete,
            7 => Self::NewFolder,
            8 => Self::NewText,
            9 => Self::CopyPath,
            10 => Self::Properties,
            11 => Self::Favorite,
            12 => Self::OpenExplorer,
            13 => Self::Reveal,
            14 => Self::Terminal,
            15 => Self::Refresh,
            _ => return None,
        })
    }
    /// 查看方式子菜单 id（0x100 段）→ token（TCP 协议用 - 分隔参数）
    fn view_token_from_id(id: u32) -> Option<&'static str> {
        let offset = id.checked_sub(MENU_CMD_BASE + 0x100)?;
        Some(match offset {
            0 => "view-details",
            1 => "view-list",
            2 => "view-columns",
            3 => "view-m-icons",
            4 => "view-l-icons",
            5 => "view-xl-icons",
            _ => return None,
        })
    }
    /// 排序方式子菜单 id（0x200 段）→ token
    fn sort_token_from_id(id: u32) -> Option<&'static str> {
        let offset = id.checked_sub(MENU_CMD_BASE + 0x200)?;
        Some(match offset {
            0 => "sort-name",
            1 => "sort-type",
            2 => "sort-modified",
            3 => "sort-size",
            _ => return None,
        })
    }
    fn token(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::OpenInOther => "open-other",
            Self::Copy => "copy",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Rename => "rename",
            Self::Delete => "delete",
            Self::NewFolder => "new-folder",
            Self::NewText => "new-text",
            Self::CopyPath => "copy-path",
            Self::Properties => "properties",
            Self::Favorite => "favorite",
            Self::OpenExplorer => "explorer",
            Self::Reveal => "reveal",
            Self::Terminal => "terminal",
            Self::Refresh => "refresh",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Open => "打开",
            Self::OpenInOther => "在另一栏打开",
            Self::Copy => "复制\tCtrl+C",
            Self::Cut => "剪切\tCtrl+X",
            Self::Paste => "粘贴\tCtrl+V",
            Self::Rename => "重命名\tF2",
            Self::Delete => "删除到回收站\tDel",
            Self::NewFolder => "新建文件夹\tCtrl+N",
            Self::NewText => "新建 TXT 文档",
            Self::CopyPath => "复制路径\tCtrl+Shift+C",
            Self::Properties => "属性",
            Self::Favorite => "收藏/取消收藏文件夹",
            Self::OpenExplorer => "在资源管理器中打开",
            Self::Reveal => "在资源管理器中定位",
            Self::Terminal => "在此处打开命令提示符",
            Self::Refresh => "刷新\tF5",
        }
    }
}

/// 自定义命令经主进程 TCP 通道回传执行。
/// Shell 菜单弹窗关闭后 helper 才发命令，此时 47719 端口必然已被主实例监听。
fn send_menu_command_to_main(command: MenuCommand, side: &str) {
    send_menu_token_to_main(command.token(), side);
}

fn send_menu_token_to_main(token: &str, side: &str) {
    let Ok(mut stream) = TcpStream::connect(FILEFLOW_COMMAND_ADDR) else {
        integration_log(&format!("menu command connect failed: {token}"));
        return;
    };
    let text = format!("fileflow-menu:{token}:{side}");
    let _ = stream.write_all(text.as_bytes());
    integration_log(&format!("menu command sent: {text}"));
}

fn run_shell_context_menu_helper(request: ShellMenuRequest) {
    integration_log(&format!(
        "shell menu helper start blank={} side={} paths={:?} third_party={}",
        request.blank,
        request.side,
        request.paths,
        request.include_third_party
    ));
    let owner_hwnd = create_shell_menu_owner();
    let result = show_windows_shell_context_menu_with_owner(&request, WHWND(owner_hwnd));
    if let Err(error) = result {
        integration_log(&format!("shell menu helper error: {error}"));
    }
    if !owner_hwnd.is_null() {
        unsafe {
            let _ = DestroyWindow(owner_hwnd);
        }
    }
    integration_log("shell menu helper end");
}

fn main() {
    // windows_subsystem="windows" 吞掉 stderr，panic 无迹可寻。
    // 把 panic 落进 integration.log，下次崩溃可诊断（05:22 拖拽中途死亡即无证据）。
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        integration_log(&format!("PANIC: {info}"));
        default_hook(info);
    }));
    let mut startup_args = std::env::args_os();
    let _ = startup_args.next();
    if startup_args.next().as_deref() == Some(std::ffi::OsStr::new("--shell-context-menu")) {
        if let Some(request) = parse_shell_menu_args(startup_args) {
            run_shell_context_menu_helper(request);
        }
        return;
    }
    trim_thumbnail_cache();
    // 部署验证标记：升级后看 integration.log 是否出现本行即可确认运行的是新构建
    integration_log("startup build 2026-10-03 blank-click-v1 shortcut-icons-v1");
    let thumbnail_result_rx = start_thumbnail_worker();
    let (watch_command_tx, directory_change_rx) = start_directory_watcher();
    let external_path = external_path_from_args();
    if !claim_single_instance_or_focus_existing(external_path.as_deref()) {
        return;
    }
    // OLE drag/drop requires the UI thread to live in an initialized STA for the
    // entire application lifetime. Initializing only when a drag begins is too late
    // for targets such as Affinity and Adobe applications.
    let ole_initialized = unsafe { OleInitialize(None).is_ok() };
    let external_command_rx = start_external_command_listener();
    let saved_win_e = fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|value| json_bool(&value, "win_e_enabled", false))
        .unwrap_or(false);
    WIN_E_ENABLED.store(saved_win_e, AtomicOrdering::Relaxed);
    if saved_win_e {
        set_win_e_startup(true);
        start_win_e_listener();
    }
    start_tray_listener();
    Application::new().run(move |cx: &mut App| {
        let startup_shortcuts = load_config(Path::new("C:\\")).shortcut_bindings;
        cx.bind_keys(app_key_bindings(&startup_shortcuts));
        // 路径状态后台校验完成 → 节流重绘（在开窗前就绪，首批渲染即可排队）
        let (ui_refresh_tx, ui_refresh_rx) = async_channel::bounded::<PathBuf>(64);
        let _ = UI_REFRESH_TX.set(ui_refresh_tx);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("FileFlow".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let watch_command_tx = watch_command_tx.clone();
                    cx.new(|cx| {
                        let app = FileFlowGpui::new(window, watch_command_tx, cx);
                        window.focus(&app.focus_handle);
                        app
                    })
                },
            )
            .unwrap();
        let view = window.update(cx, |_, _, cx| cx.entity()).unwrap();
        start_external_command_pump(window, view.clone(), external_command_rx, cx);
        start_directory_change_pump(view.clone(), directory_change_rx, cx);
        start_thumbnail_result_pump(view.clone(), thumbnail_result_rx, cx);
        start_thumbnail_result_pump(view.clone(), ui_refresh_rx, cx);
        let folder_size_done_rx = start_folder_size_worker();
        start_thumbnail_result_pump(view.clone(), folder_size_done_rx, cx);
        view.update(cx, |app, cx| app.reload_visible_async(cx));
        // subclass 顶层窗口：drag 区 WM_NCHITTEST → HTCAPTION（窗口拖动）
        install_drag_zone_subclass();
        if let Some(path) = external_path.clone() {
            view.update(cx, |app, cx| app.open_external_path(path, cx));
        }
        let shortcut_view = view.clone();
        cx.intercept_keystrokes(move |event, _, cx| {
            let recording = shortcut_view.read(cx).shortcut_recording.is_some();
            if !recording {
                return;
            }
            cx.stop_propagation();
            let binding = keystroke_binding(&event.keystroke);
            let updated = shortcut_view.update(cx, |app, cx| {
                let id = app.shortcut_recording.take()?;
                let Some(binding) = binding else {
                    app.status = "这个按键不能用作快捷键".to_string();
                    cx.notify();
                    return None;
                };
                if binding == "escape" {
                    app.status = "已取消快捷键录制".to_string();
                    cx.notify();
                    return None;
                }
                let conflict = shortcut_specs().iter().find(|spec| {
                    spec.id != id && shortcut_key(&app.shortcut_bindings, spec.id) == binding
                });
                if let Some(conflict) = conflict {
                    app.status = format!("快捷键已被“{}”使用", conflict.label);
                    cx.notify();
                    return None;
                }
                app.shortcut_bindings.insert(id, binding.clone());
                app.status = format!("快捷键已设为 {}", display_shortcut(&binding));
                app.save_config();
                cx.notify();
                Some(app.shortcut_bindings.clone())
            });
            if let Some(bindings) = updated {
                cx.clear_key_bindings();
                cx.bind_keys(app_key_bindings(&bindings));
            }
        })
        .detach();
        cx.observe_keystrokes(move |event, window, cx| {
            let Some(text) = event
                .keystroke
                .key_char
                .as_ref()
                .or(Some(&event.keystroke.key))
            else {
                return;
            };
            if event.action.is_some()
                || event.keystroke.modifiers.control
                || event.keystroke.modifiers.alt
                || event.keystroke.modifiers.platform
                || event.keystroke.modifiers.function
                || text.chars().count() != 1
                || text.trim().is_empty()
            {
                return;
            }
            let text = text.to_string();
            let skip = {
                let app = view.read(cx);
                app.filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                    || app
                        .right_filter_input
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || app
                        .address_input
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || app
                        .new_folder_input
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || app
                        .rename_input
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || app.new_folder_open
                    || app.rename_open
                    || app.show_settings
                    || app.show_shortcuts
            };
            if skip {
                return;
            }
            view.update(cx, |app, cx| app.select_by_prefix(&text, cx));
        })
        .detach();
        cx.activate(true);
        cache_fileflow_hwnd();
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(300));
            cache_fileflow_hwnd();
        });
    });
    if ole_initialized {
        unsafe {
            OleUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_external_file_urls_and_quoted_unc_paths() {
        assert_eq!(
            clean_external_path_arg("file:///C:/Users/Test/Pictures"),
            Some(PathBuf::from("C:\\Users\\Test\\Pictures"))
        );
        assert_eq!(
            clean_external_path_arg("\"\\\\server\\share\\folder\""),
            Some(PathBuf::from("\\\\server\\share\\folder"))
        );
        // 盘符 + 正斜杠/双斜杠变体也要归一化（helper 的 --paths 参数来源不可控）
        assert_eq!(
            clean_external_path_arg("C://Windows"),
            Some(PathBuf::from("C:\\Windows"))
        );
        assert_eq!(
            clean_external_path_arg("C:/Users//Test"),
            Some(PathBuf::from("C:\\Users\\Test"))
        );
    }

    #[test]
    fn parses_shell_menu_commands_over_tcp_protocol() {
        for (text, command, side) in [
            ("fileflow-menu:copy:left", "copy", "left"),
            ("fileflow-menu:delete:right", "delete", "right"),
            ("fileflow-menu:new-folder:left", "new-folder", "left"),
            ("fileflow-menu:refresh:right", "refresh", "right"),
            // 子菜单带参数 token
            ("fileflow-menu:view-details:left", "view-details", "left"),
            ("fileflow-menu:view-xl-icons:right", "view-xl-icons", "right"),
            ("fileflow-menu:sort-name:left", "sort-name", "left"),
            ("fileflow-menu:sort-size:right", "sort-size", "right"),
        ] {
            match parse_external_command(text) {
                Some(ExternalCommand::Menu { command: c, side: s }) => {
                    assert_eq!(c, command, "command mismatch for {text}");
                    assert_eq!(s, side, "side mismatch for {text}");
                }
                other => panic!("expected menu command for {text}, got {other:?}"),
            }
        }
        assert!(parse_external_command("fileflow-menu:unknown-cmd:left").is_none());
        // 普通路径仍走原通道
        assert!(matches!(
            parse_external_command("C:\\Users"),
            Some(ExternalCommand::Path(_))
        ));
    }

    #[test]
    fn submenu_command_ids_map_to_tokens() {
        let base = MENU_CMD_BASE;
        assert_eq!(
            MenuCommand::view_token_from_id(base + 0x100),
            Some("view-details")
        );
        assert_eq!(
            MenuCommand::view_token_from_id(base + 0x105),
            Some("view-xl-icons")
        );
        assert_eq!(MenuCommand::view_token_from_id(base + 0x106), None);
        assert_eq!(
            MenuCommand::sort_token_from_id(base + 0x200),
            Some("sort-name")
        );
        assert_eq!(
            MenuCommand::sort_token_from_id(base + 0x203),
            Some("sort-size")
        );
        assert_eq!(MenuCommand::sort_token_from_id(base + 0x204), None);
        // 子菜单 id 不能误判为普通命令，也不能落入 Shell id 范围
        assert!(MenuCommand::from_id(base + 0x100).is_none());
    }

    #[test]
    fn menu_command_ids_round_trip_and_do_not_collide_with_shell_range() {
        for offset in 0..16u32 {
            let Some(command) = MenuCommand::from_id(MENU_CMD_BASE + offset) else {
                panic!("offset {offset} should map to a command");
            };
            assert_eq!(command.id(), MENU_CMD_BASE + offset);
            assert!(command.id() >= MENU_CMD_BASE);
        }
        assert!(
            MenuCommand::from_id(MENU_CMD_BASE + 0x100).is_none(),
            "子菜单 id 不应映射为命令"
        );
        assert!(MenuCommand::from_id(1).is_none());
    }

    #[test]
    fn detects_only_unc_server_roots() {
        assert!(is_unc_server_root(Path::new("\\\\192.168.2.109")));
        assert!(!is_unc_server_root(Path::new("\\\\192.168.2.109\\media")));
        assert!(!is_unc_server_root(Path::new("D:\\media")));
    }

    #[test]
    fn normalizes_drive_relative_paths_for_shell_integration() {
        assert_eq!(
            normalize_path(PathBuf::from("E:z照片\\image.png")),
            PathBuf::from("E:\\z照片\\image.png")
        );
        assert_eq!(
            normalize_path(PathBuf::from("E:\\z照片\\image.png")),
            PathBuf::from("E:\\z照片\\image.png")
        );
    }

    #[test]
    fn shortcut_defaults_are_unique_and_parseable() {
        let mut keys = HashSet::new();
        for spec in shortcut_specs() {
            assert!(Keystroke::parse(spec.default_key).is_ok(), "{}", spec.id);
            assert!(
                keys.insert(spec.default_key),
                "duplicate {}",
                spec.default_key
            );
        }
    }

    #[test]
    fn shortcut_display_is_user_friendly() {
        assert_eq!(display_shortcut("ctrl-shift-c"), "Ctrl+Shift+C");
        assert_eq!(display_shortcut("alt-left"), "Alt+Left");
    }

    #[test]
    fn invalid_or_unknown_shortcuts_are_ignored() {
        let value = serde_json::json!({
            "shortcut_bindings": {
                "refresh": "ctrl-alt-r",
                "not_an_action": "ctrl-z",
                "copy": "not-a-valid-key-name-part"
            }
        });
        let parsed = parsed_shortcut_bindings(&value);
        assert_eq!(
            parsed.get("refresh").map(String::as_str),
            Some("ctrl-alt-r")
        );
        assert!(!parsed.contains_key("not_an_action"));
        assert!(!parsed.contains_key("copy"));
    }

    #[test]
    fn rectangle_selection_uses_intersection() {
        assert!(rect_intersects((0., 0., 20., 20.), (19., 19., 40., 40.)));
        assert!(!rect_intersects((0., 0., 20., 20.), (21., 21., 40., 40.)));
    }

    #[test]
    fn batch_rename_numbers_and_keeps_extensions() {
        let dir = std::env::temp_dir().join(format!(
            "fileflow-batch-rename-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("photo one.jpg");
        let b = dir.join("photo two.png");
        let c = dir.join("notes.txt");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        fs::write(&c, b"c").unwrap();
        let (renamed, errors) = batch_rename_files(&[a.clone(), b.clone(), c.clone()], "trip", 1);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(renamed.len(), 3);
        assert_eq!(renamed[0].file_name().unwrap(), "trip_001.jpg");
        assert_eq!(renamed[1].file_name().unwrap(), "trip_002.png");
        assert_eq!(renamed[2].file_name().unwrap(), "trip_003.txt");
        // 目标名已存在时跳过不覆盖
        fs::write(dir.join("trip_001.log"), b"keep").unwrap();
        let source = dir.join("other.log");
        fs::write(&source, b"x").unwrap();
        let (renamed2, errors2) = batch_rename_files(std::slice::from_ref(&source), "trip", 1);
        assert_eq!(renamed2.len(), 0);
        assert_eq!(errors2.len(), 1);
        assert_eq!(fs::read(dir.join("trip_001.log")).unwrap(), b"keep");
        assert_eq!(fs::read(&source).unwrap(), b"x");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn thumbnail_cache_key_changes_with_file_metadata() {
        let path = std::env::temp_dir().join(format!(
            "fileflow-thumbnail-test-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::write(&path, b"one").unwrap();
        let first = {
            let metadata = fs::metadata(&path).unwrap();
            thumbnail_cache_path_for(&path, metadata.len(), 1)
        };
        fs::write(&path, b"a larger replacement").unwrap();
        let second = {
            let metadata = fs::metadata(&path).unwrap();
            thumbnail_cache_path_for(&path, metadata.len(), 2)
        };
        let _ = fs::remove_file(path);
        assert_ne!(first, second);
    }

    /// 实测 webp 缩略图管线：缓存 PNG 必须是真实内容（而非文件类型图标）。
    /// 样本缺失时跳过（不视为失败）。
    #[test]
    fn generates_webp_thumbnail_via_shell_pipeline() {
        // fixture 相对仓库根（tests/fixtures/sample.webp），无个人路径
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.webp");
        if !sample.is_file() {
            eprintln!("sample webp missing, skipped: {}", sample.display());
            return;
        }
        let temp_dir = std::env::temp_dir().join(format!(
            "fileflow-shell-thumb-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let copy = temp_dir.join("sample.webp");
        fs::copy(&sample, &copy).unwrap();

        // shell 管线（THUMBNAILONLY）：有真缩略图就用，没有返回 None 也合法
        let shell = shell_thumbnail(&copy);
        eprintln!("shell_thumbnail returned: {}", shell.is_some());

        let cached = generate_cached_thumbnail(&copy).expect("cache generation failed");
        assert!(cached.is_file(), "cache png missing: {}", cached.display());
        let reloaded = image::open(&cached).expect("cache png not decodable");
        assert!(reloaded.width() > 0 && reloaded.height() > 0);
        if shell.is_none() {
            // image crate 路径：缓存内容必须与源图解码一致（防止把类型图标当缩略图）
            let direct = image::open(&copy)
                .expect("direct decode failed")
                .thumbnail(256, 256);
            assert_eq!(
                (reloaded.width(), reloaded.height()),
                (direct.width(), direct.height())
            );
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore]
    fn publishes_shell_file_clipboard() {
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        put_files_on_windows_clipboard(&[sample], DROPEFFECT_COPY).unwrap();
    }

    /// 回归：filetype 图标必须落盘缓存。曾因 GetDIBits 查询头（预置
    /// biBitCount=32 的 cLines=0 调用）恒返回 0，缓存永远写不进去，
    /// 每帧对每个文件重新提取，大目录（900+ 项）卡到秒级。
    #[test]
    fn filetype_icon_cache_persists_png() {
        let icon = cached_filetype_icon_path(Path::new(r"C://probe-not-exist.zip"), 70.)
            .expect("filetype icon extraction failed");
        assert!(icon.is_file(), "cache png missing: {}", icon.display());
        // 会话记忆：同键再次命中不应再触发提取
        let again = cached_filetype_icon_path(Path::new(r"C://probe-not-exist2.zip"), 70.);
        assert_eq!(again.as_deref(), Some(icon.as_path()));
    }

    #[test]
    fn multiline_new_item_names_preserve_unicode_and_add_txt_once() {
        assert_eq!(new_item_names("项目甲\r\n\r\n 项目乙 \n项目丙", NewItemKind::Folder).unwrap(), vec!["项目甲", "项目乙", "项目丙"]);
        assert_eq!(new_item_names("笔记\nREADME.TXT\n草稿.md", NewItemKind::TextFile).unwrap(), vec!["笔记.txt", "README.TXT", "草稿.md.txt"]);
        for invalid in ["", " \n\n", "../escape", "..", "D:\\outside", "CON.txt", "COM1", "name."] {
            assert!(new_item_names(invalid, NewItemKind::Folder).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn batch_creation_creates_all_lines_without_overwriting_existing_files() {
        let base = std::env::temp_dir().join(format!("fileflow-create-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let names = new_item_names("项目甲\n项目乙\n项目甲", NewItemKind::Folder).unwrap();
        let (folders, errors) = create_new_items(&base, &names, NewItemKind::Folder);
        assert!(errors.is_empty());
        assert_eq!(folders.len(), 3);
        assert!(folders.iter().all(|path| path.is_dir()));
        fs::write(base.join("笔记.txt"), "保留原内容").unwrap();
        let names = new_item_names("笔记\n清单.txt\n笔记", NewItemKind::TextFile).unwrap();
        let (files, errors) = create_new_items(&base, &names, NewItemKind::TextFile);
        assert!(errors.is_empty());
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|path| path.extension().is_some_and(|ext| ext == "txt") && fs::read(path).unwrap().is_empty()));
        assert_eq!(fs::read_to_string(base.join("笔记.txt")).unwrap(), "保留原内容");
        for file in files { fs::remove_file(file).unwrap(); }
        fs::remove_file(base.join("笔记.txt")).unwrap();
        for folder in folders { fs::remove_dir(folder).unwrap(); }
        fs::remove_dir(base).unwrap();
    }

    #[test]
    fn shortcut_icons_use_each_shortcuts_shell_icon() {
        use windows::Win32::System::Com::IPersistFile;
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        use windows_core::Interface;

        let directory = std::env::temp_dir().join(format!("fileflow-shortcuts-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let shell_library = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("System32/shell32.dll");
        let library_wide: Vec<u16> = shell_library.as_os_str().encode_wide().chain(Some(0)).collect();
        let shortcuts = [directory.join("folder.lnk"), directory.join("drive.lnk")];
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            for (path, icon_index) in shortcuts.iter().zip([3, 8]) {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
                link.SetPath(WPCWSTR(library_wide.as_ptr())).unwrap();
                link.SetIconLocation(WPCWSTR(library_wide.as_ptr()), icon_index).unwrap();
                let persist: IPersistFile = link.cast().unwrap();
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                persist.Save(WPCWSTR(wide.as_ptr()), true).unwrap();
            }
            let entries = read_entries(&directory);
            assert!(shortcuts.iter().all(|path| entries.iter().any(|entry| &entry.path == path)));
            let first = cached_filetype_icon_path(&shortcuts[0], 70.).unwrap();
            let second = cached_filetype_icon_path(&shortcuts[1], 70.).unwrap();
            assert_ne!(first, second, "shortcuts must not share the extension cache");
            let first_image = image::open(first).unwrap().to_rgba8();
            let second_image = image::open(second).unwrap().to_rgba8();
            assert!(first_image.pixels().any(|pixel| pixel[3] != 0));
            assert_ne!(first_image, second_image, "custom shortcut icons must be preserved");
            CoUninitialize();
        }
        for path in shortcuts {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
}
