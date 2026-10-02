#![windows_subsystem = "windows"]

use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn main() {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("FileFlow.exe")))
        .unwrap_or_else(|| PathBuf::from("FileFlow.exe"));
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut command = Command::new(exe);
    command.args(args);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let _ = command.spawn();
}
