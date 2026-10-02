use std::path::PathBuf;

fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        // winresource 默认用 reg.exe 查注册表定位 Windows SDK 的 rc.exe；
        // 本机 reg.exe 被安全策略拦截时探测失败 → 图标嵌入静默失败（exe 无图标）。
        // RC_PATH 环境变量可跳过探测直接指定 rc.exe，优先级最高。
        if std::env::var_os("RC_PATH").is_none() {
            let kit = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
            if let Some(rc) = latest_rc_in_kit(&kit) {
                // SAFETY: build.rs 单线程执行，无并发读取环境变量
                unsafe { std::env::set_var("RC_PATH", rc) };
            }
        }
        resource.set_icon("Assets/FileFlow.ico");
        resource.set("FileDescription", "FileFlow");
        resource.set("ProductName", "FileFlow");
        resource.set("OriginalFilename", "FileFlow.exe");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=failed to embed FileFlow icon: {error}");
        }
    }
}

/// 在 Windows Kits 的 bin 目录下找最高版本号的 x64\rc.exe
#[cfg(windows)]
fn latest_rc_in_kit(kit: &PathBuf) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(kit).ok()? {
        let entry = entry.ok()?;
        let path = entry.path().join("x64").join("rc.exe");
        if !path.is_file() {
            continue;
        }
        let mut version = 0u64;
        for part in entry.file_name().to_string_lossy().split('.') {
            if let Ok(number) = part.parse::<u64>() {
                version = version * 1000 + number;
            }
        }
        if best.as_ref().is_none_or(|(current, _)| version > *current) {
            best = Some((version, path));
        }
    }
    best.map(|(_, path)| path)
}
