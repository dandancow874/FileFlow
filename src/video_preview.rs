//! Windows Media Engine renders into a non-focusable child HWND. GPUI only draws
//! controls: no per-frame CPU image copies and no polling when preview is closed.
use gpui::{
    Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render,
    Window, canvas, div, prelude::*, px, relative, rgb,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HMODULE, RECT},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device, ID3D11Multithread,
            },
        },
        Media::MediaFoundation::*,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
    },
    core::{BSTR, Interface, Result, implement},
};
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{BLACK_BRUSH, GetStockObject},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, IsIconic, IsWindowVisible,
        RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
        ShowWindow, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_DISABLED, WS_EX_NOACTIVATE,
        WS_EX_NOPARENTNOTIFY,
    },
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

pub fn is_video_file(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "mp4"
                | "m4v"
                | "mov"
                | "mkv"
                | "avi"
                | "wmv"
                | "webm"
                | "mpg"
                | "mpeg"
                | "mts"
                | "m2ts"
                | "ts"
                | "3gp"
        )
    })
}

#[derive(Default)]
struct Events {
    error: Option<String>,
}

#[implement(IMFMediaEngineNotify)]
struct MediaNotify {
    events: Arc<Mutex<Events>>,
}
impl IMFMediaEngineNotify_Impl for MediaNotify_Impl {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> Result<()> {
        // Callback is on MF threads. Never call GPUI or the engine here (re-entry).
        if event == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32
            && let Ok(mut events) = self.events.lock()
        {
            events.error = Some(format!(
                "无法预览此视频，系统可能缺少解码器（错误 {param1} / 0x{param2:08X}）"
            ));
        }
        Ok(())
    }
}

struct Runtime {
    com_initialized: bool,
}
impl Runtime {
    fn new() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // GPUI may have already initialized this thread with a different model.
        if hr.is_err() && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            hr.ok()?;
        }
        let com_initialized = hr.is_ok();
        if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
            if com_initialized {
                unsafe { CoUninitialize() };
            }
            return Err(e);
        }
        Ok(Self { com_initialized })
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}

struct VideoWindow(HWND);
impl VideoWindow {
    fn new(parent: HWND) -> Result<Self> {
        let class: Vec<u16> = "FileFlowVideoSurface\0".encode_utf16().collect();
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                hbrBackground: GetStockObject(BLACK_BRUSH) as _,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            // Already registered is expected after the first preview.
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_NOPARENTNOTIFY,
                class.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_DISABLED | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return Err(windows::core::Error::from_win32());
            }
            Ok(Self(hwnd))
        }
    }
}
impl Drop for VideoWindow {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

struct Player {
    engine: IMFMediaEngine,
    extended: IMFMediaEngineEx,
    surface: VideoWindow,
    events: Arc<Mutex<Events>>,
    parent: HWND,
    rect: Option<(i32, i32, i32, i32)>,
    aspect: (u32, u32),
    visible: bool,
    // Retain the device manager until engine shutdown. Drop runtime last.
    _device_manager: Option<IMFDXGIDeviceManager>,
    _runtime: Runtime,
}
impl Player {
    fn open(path: &Path, parent: usize) -> Result<Self> {
        let runtime = Runtime::new()?;
        let parent = parent as HWND;
        let surface = VideoWindow::new(parent)?;
        let events = Arc::new(Mutex::new(Events::default()));
        let callback: IMFMediaEngineNotify = MediaNotify {
            events: events.clone(),
        }
        .into();
        let mut attrs = None;
        unsafe {
            MFCreateAttributes(&mut attrs, 3)?;
        }
        let attrs = attrs.expect("MFCreateAttributes succeeded");
        unsafe {
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &callback)?;
            attrs.SetUINT64(&MF_MEDIA_ENGINE_PLAYBACK_HWND, surface.0 as u64)?;
        }
        let manager = hardware_device_manager().ok();
        if let Some(manager) = &manager {
            unsafe {
                attrs.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, manager)?;
            }
        }
        let factory: IMFMediaEngineClassFactory = unsafe {
            CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)?
        };
        let engine = unsafe { factory.CreateInstance(0, &attrs)? };
        let extended = match engine.cast::<IMFMediaEngineEx>() {
            Ok(ex) => ex,
            Err(e) => {
                unsafe {
                    let _ = engine.Shutdown();
                }
                return Err(e);
            }
        };
        let player = Self {
            engine,
            extended,
            surface,
            events,
            parent,
            rect: None,
            aspect: (0, 0),
            visible: false,
            _device_manager: manager,
            _runtime: runtime,
        };
        // Source resolution/decoding is asynchronous. A full local/UNC path, not
        // an escaped URL, also handles Chinese names, spaces and '#' literally.
        unsafe {
            player.engine.SetAutoPlay(true)?;
            player
                .engine
                .SetSource(&BSTR::from(path.to_string_lossy().as_ref()))?;
            player.engine.Load()?;
        }
        crate::integration_log(if player._device_manager.is_some() {
            "video-preview opened native media engine with D3D11 hardware device"
        } else {
            "video-preview opened native media engine with system default device"
        });
        Ok(player)
    }
    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        unsafe {
            ShowWindow(
                self.surface.0,
                if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
            );
        }
        self.visible = visible;
    }
    fn place(&mut self, bounds: Bounds<Pixels>, scale: f32, blocked: bool) {
        let rect = (
            (f32::from(bounds.origin.x) * scale).round() as i32,
            (f32::from(bounds.origin.y) * scale).round() as i32,
            (f32::from(bounds.size.width) * scale).round().max(1.) as i32,
            (f32::from(bounds.size.height) * scale).round().max(1.) as i32,
        );
        let mut aspect = (0, 0);
        unsafe {
            let _ = self
                .engine
                .GetVideoAspectRatio(Some(&mut aspect.0), Some(&mut aspect.1));
        }
        if self.rect != Some(rect) || self.aspect != aspect {
            unsafe {
                SetWindowPos(
                    self.surface.0,
                    std::ptr::null_mut(),
                    rect.0,
                    rect.1,
                    rect.2,
                    rect.3,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
                let dst = contain_rect(rect.2, rect.3, aspect);
                let _ = self.extended.UpdateVideoStream(None, Some(&dst), None);
            }
            self.rect = Some(rect);
            self.aspect = aspect;
        }
        let ready = unsafe { self.engine.GetReadyState() >= 2 };
        self.set_visible(!blocked && ready);
    }
    fn pause_when_hidden(&mut self, blocked: bool) {
        let hidden = blocked
            || unsafe {
                IsIconic(self.parent) != 0
                    || IsWindowVisible(self.parent) == 0
                    || IsWindowEnabled(self.parent) == 0
            };
        if hidden {
            unsafe {
                if self.engine.GetAutoPlay().as_bool() {
                    let _ = self.engine.SetAutoPlay(false);
                }
                if !self.engine.IsPaused().as_bool() {
                    let _ = self.engine.Pause();
                }
            }
            self.set_visible(false);
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        // Shutdown stops MF work and audio BEFORE destroying HWND/runtime.
        unsafe {
            let _ = self.engine.Pause();
            let _ = self.engine.Shutdown();
        }
        crate::integration_log("video-preview shutdown and resources released");
    }
}

fn hardware_device_manager() -> Result<IMFDXGIDeviceManager> {
    unsafe {
        let mut device: Option<ID3D11Device> = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
        let device = device.expect("D3D11CreateDevice succeeded");
        // MF uses this device from several threads.
        let multithread = device.GetImmediateContext()?.cast::<ID3D11Multithread>()?;
        let _ = multithread.SetMultithreadProtected(true);
        let mut token = 0;
        let mut manager = None;
        MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
        let manager = manager.expect("MFCreateDXGIDeviceManager succeeded");
        manager.ResetDevice(&device, token)?;
        Ok(manager)
    }
}

#[derive(Clone, Default, PartialEq)]
struct Playback {
    position: f64,
    duration: f64,
    paused: bool,
    ready: bool,
    ended: bool,
    muted: bool,
    hidden: bool,
    error: Option<String>,
}
fn contain_rect(width: i32, height: i32, aspect: (u32, u32)) -> RECT {
    if aspect.0 == 0 || aspect.1 == 0 {
        return RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
    }
    let scale = (width as f64 / aspect.0 as f64).min(height as f64 / aspect.1 as f64);
    let video_width = (aspect.0 as f64 * scale).round() as i32;
    let video_height = (aspect.1 as f64 * scale).round() as i32;
    let left = (width - video_width) / 2;
    let top = (height - video_height) / 2;
    RECT {
        left,
        top,
        right: left + video_width,
        bottom: top + video_height,
    }
}
fn finite_time(time: f64) -> f64 {
    if time.is_finite() { time.max(0.) } else { 0. }
}
fn time_label(time: f64) -> String {
    let secs = finite_time(time) as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

pub struct VideoPreview {
    target: Option<PathBuf>,
    player: Rc<RefCell<Option<Player>>>,
    playback: Playback,
    generation: u64,
    blocked: bool,
    seeking: bool,
    seek_bounds: Rc<RefCell<Bounds<Pixels>>>,
}
impl VideoPreview {
    pub fn new() -> Self {
        Self {
            target: None,
            player: Rc::new(RefCell::new(None)),
            playback: Playback::default(),
            generation: 0,
            blocked: false,
            seeking: false,
            seek_bounds: Rc::new(RefCell::new(Bounds::default())),
        }
    }
    pub fn set_target(
        &mut self,
        target: Option<PathBuf>,
        parent: usize,
        blocked: bool,
        cx: &mut Context<Self>,
    ) {
        let blocked_changed = self.blocked != blocked;
        self.blocked = blocked;
        if self.target == target {
            if let Some(player) = self.player.borrow_mut().as_mut() {
                player.pause_when_hidden(blocked);
            }
            if blocked_changed {
                cx.notify();
            }
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.player.borrow_mut().take();
        self.target = target.clone();
        self.playback = Playback::default();
        self.seeking = false;
        cx.notify();
        let Some(path) = target else {
            return;
        };
        let generation = self.generation;
        // Initialize outside the current render; don't synchronously create a
        // child HWND while GPUI is traversing its render tree.
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            crate::wait_out_of_ole_modal(cx).await;
            let result = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return false;
                }
                match Player::open(&path, parent) {
                    Ok(mut player) => {
                        player.pause_when_hidden(this.blocked);
                        *this.player.borrow_mut() = Some(player);
                    }
                    Err(error) => {
                        this.playback.error = Some(format!("视频预览不可用：{error}"));
                    }
                }
                cx.notify();
                true
            });
            if !matches!(result, Ok(true)) {
                return;
            }
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                crate::wait_out_of_ole_modal(cx).await;
                let alive = this.update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    this.refresh(cx);
                    this.player.borrow().is_some()
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let next = {
            let mut slot = self.player.borrow_mut();
            let Some(player) = slot.as_mut() else {
                return;
            };
            player.pause_when_hidden(self.blocked);
            let error = player.events.lock().ok().and_then(|e| e.error.clone());
            unsafe {
                Playback {
                    position: (finite_time(player.engine.GetCurrentTime()) * 4.).floor() / 4.,
                    duration: finite_time(player.engine.GetDuration()),
                    paused: player.engine.IsPaused().as_bool(),
                    ready: player.engine.GetReadyState() >= 2,
                    ended: player.engine.IsEnded().as_bool(),
                    muted: player.engine.GetMuted().as_bool(),
                    hidden: self.blocked
                        || IsIconic(player.parent) != 0
                        || IsWindowVisible(player.parent) == 0
                        || IsWindowEnabled(player.parent) == 0,
                    error,
                }
            }
        };
        if next.error.is_some() {
            self.player.borrow_mut().take();
        }
        if next != self.playback {
            self.playback = next;
            cx.notify();
        }
    }
    fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.player.borrow().as_ref() {
            unsafe {
                if player.engine.IsPaused().as_bool() || player.engine.IsEnded().as_bool() {
                    if player.engine.IsEnded().as_bool() {
                        let _ = player.engine.SetCurrentTime(0.);
                    }
                    let _ = player.engine.Play();
                } else {
                    let _ = player.engine.Pause();
                }
            }
        }
        self.refresh(cx);
    }
    fn seek_to(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if !self.playback.ready || self.playback.duration <= 0. {
            return;
        }
        let bounds = *self.seek_bounds.borrow();
        let width = f32::from(bounds.size.width);
        if width <= 0. {
            return;
        }
        let fraction = ((f32::from(x) - f32::from(bounds.origin.x)) / width).clamp(0., 1.);
        if let Some(player) = self.player.borrow().as_ref() {
            unsafe {
                let _ = player
                    .engine
                    .SetCurrentTime(self.playback.duration * fraction as f64);
            }
        }
        self.refresh(cx);
    }
}

impl Render for VideoPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let player = self.player.clone();
        let blocked = self.blocked;
        let seek_bounds = self.seek_bounds.clone();
        let position = self.playback.position;
        let duration = self.playback.duration;
        let fraction = if duration > 0. {
            (position / duration).clamp(0., 1.) as f32
        } else {
            0.
        };
        let title = self
            .target
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let body = if let Some(error) = &self.playback.error {
            div()
                .flex_1()
                .p_4()
                .text_color(rgb(0x777f86))
                .child(error.clone())
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .bg(rgb(0x000000))
                .child(
                    canvas(
                        move |bounds, window, _| {
                            if let Some(player) = player.borrow_mut().as_mut() {
                                player.place(bounds, window.scale_factor(), blocked);
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .size_full(),
                )
                .when(!self.playback.ready, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top_4()
                            .left_4()
                            .text_color(rgb(0xeeeeee))
                            .child("正在加载视频…"),
                    )
                })
                .into_any_element()
        };
        div()
            .id("native-video-preview")
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(12.))
                    .overflow_hidden()
                    .child(title),
            )
            .child(body)
            .child(
                div()
                    .id("video-controls")
                    .flex_none()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("video-seek")
                            .relative()
                            .h(px(18.))
                            .w_full()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.seeking = true;
                                    this.seek_to(event.position.x, cx);
                                }),
                            )
                            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                                if this.seeking && event.pressed_button == Some(MouseButton::Left) {
                                    cx.stop_propagation();
                                    this.seek_to(event.position.x, cx);
                                }
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, _, cx| {
                                    cx.stop_propagation();
                                    if this.seeking {
                                        this.seek_to(event.position.x, cx);
                                    }
                                    this.seeking = false;
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.seeking = false),
                            )
                            .child(
                                canvas(
                                    move |bounds, _, _| *seek_bounds.borrow_mut() = bounds,
                                    |_, _, _, _| {},
                                )
                                .size_full(),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(7.))
                                    .h(px(4.))
                                    .w_full()
                                    .rounded_sm()
                                    .bg(rgb(0xd9dfe3)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(7.))
                                    .h(px(4.))
                                    .w(relative(fraction))
                                    .rounded_sm()
                                    .bg(rgb(0x009bdb)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_size(px(13.))
                            // mouse_down is intentional: GPUI on_click can be lost across
                            // the position timer's repaint between down/up.
                            .child(
                                div()
                                    .id("video-play-pause")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0xe9eef2))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.toggle_play(cx);
                                        }),
                                    )
                                    .child(if self.playback.ended {
                                        "重播"
                                    } else if self.playback.paused {
                                        "播放"
                                    } else {
                                        "暂停"
                                    }),
                            )
                            .child(format!(
                                "{} / {}",
                                time_label(position),
                                time_label(duration)
                            ))
                            .child(
                                div()
                                    .id("video-mute")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_1()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            if let Some(player) = this.player.borrow().as_ref() {
                                                unsafe {
                                                    let _ = player.engine.SetMuted(
                                                        !player.engine.GetMuted().as_bool(),
                                                    );
                                                }
                                            }
                                            this.refresh(cx);
                                        }),
                                    )
                                    .child(if self.playback.muted {
                                        "取消静音"
                                    } else {
                                        "静音"
                                    }),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn video_types_and_times_are_bounded() {
        assert!(is_video_file(Path::new("中文.MP4")));
        assert!(is_video_file(Path::new("clip.mkv")));
        assert!(!is_video_file(Path::new("photo.png")));
        assert_eq!(time_label(f64::NAN), "0:00");
        assert_eq!(time_label(f64::INFINITY), "0:00");
        assert_eq!(time_label(3661.), "1:01:01");
        let landscape = contain_rect(600, 600, (1920, 1080));
        assert_eq!(
            (
                landscape.left,
                landscape.top,
                landscape.right,
                landscape.bottom
            ),
            (0, 131, 600, 469)
        );
        let portrait = contain_rect(600, 400, (1080, 1920));
        assert_eq!(
            (portrait.left, portrait.top, portrait.right, portrait.bottom),
            (187, 0, 412, 400)
        );
    }

    /// Explicit native playback check, using a caller-supplied local fixture.
    /// It owns an invisible test HWND and never manipulates any user window.
    #[test]
    #[ignore = "requires FILEFLOW_VIDEO_TEST_PATH and installed Windows media components"]
    fn native_engine_loads_plays_seeks_pauses_and_releases_file() {
        use std::{os::windows::fs::OpenOptionsExt, time::Instant};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, IsWindow, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WS_POPUP,
        };
        let path = PathBuf::from(
            std::env::var_os("FILEFLOW_VIDEO_TEST_PATH").expect("video test fixture"),
        );
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let parent = VideoWindow(unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                800,
                600,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        });
        assert!(!parent.0.is_null());
        let mut player = Player::open(&path, parent.0 as usize).expect("native engine");
        let child = player.surface.0;
        let wait = |condition: &dyn Fn() -> bool| {
            let start = Instant::now();
            while !condition() && start.elapsed() < Duration::from_secs(15) {
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(condition(), "native engine did not reach expected state");
        };
        wait(&|| unsafe { player.engine.GetReadyState() >= 2 });
        assert!(player.events.lock().unwrap().error.is_none());
        let duration = unsafe { player.engine.GetDuration() };
        assert!(duration > 2. && duration.is_finite());
        unsafe {
            player.engine.Play().unwrap();
        }
        let before = unsafe { player.engine.GetCurrentTime() };
        wait(&|| unsafe { player.engine.GetCurrentTime() > before + 0.15 });
        unsafe {
            player.engine.Pause().unwrap();
        }
        wait(&|| unsafe { player.engine.IsPaused().as_bool() });
        let paused = unsafe { player.engine.GetCurrentTime() };
        std::thread::sleep(Duration::from_millis(300));
        assert!((unsafe { player.engine.GetCurrentTime() } - paused).abs() < 0.1);
        unsafe {
            player.engine.SetCurrentTime(duration / 2.).unwrap();
        }
        wait(&|| unsafe {
            !player.engine.IsSeeking().as_bool()
                && (player.engine.GetCurrentTime() - duration / 2.).abs() < 1.
        });
        unsafe {
            player.engine.Play().unwrap();
        }
        player.pause_when_hidden(false);
        wait(&|| unsafe { player.engine.IsPaused().as_bool() });
        drop(player);
        assert_eq!(
            unsafe { IsWindow(child) },
            0,
            "native surface must be destroyed on close"
        );
        wait(&|| {
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&path)
                .is_ok()
        });
    }
}
