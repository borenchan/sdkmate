//! 平台相关小工具：管理员检测与提权重启。
//!
//! Windows 下 init/switch 需要写系统 PATH/环境变量，首次初始化前需要管理员权限；
//! 非 Windows 平台默认视为已具备权限（写用户目录与 shell profile）。

#[cfg(windows)]
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// 当前进程是否具备管理员权限（非 Windows 恒为 true）
#[cfg(windows)]
pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}

/// 以管理员身份重启当前 GUI（Windows UAC 弹窗）；成功返回 true
#[cfg(windows)]
pub fn relaunch_as_admin() -> bool {
    use std::ffi::OsStr;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let operation = wide(OsStr::new("runas"));
    let file = wide(exe.as_os_str());
    // u32 以下为错误码，>32 表示 ShellExecute 成功发起
    let rc = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    rc as isize > 32
}

/// 用系统默认程序打开文件/目录
///
/// Windows 走 ShellExecuteW（shell 亲自处理，不拉起 cmd，避免控制台闪窗）；
/// macOS/Linux 沿用 open/xdg-open。
#[cfg(windows)]
pub fn open_path(path: &str) {
    use std::ffi::OsStr;

    let operation = wide(OsStr::new("open"));
    let file = wide(OsStr::new(path));
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(target_os = "macos")]
pub fn open_path(path: &str) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn open_path(path: &str) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// 转为以 NUL 结尾的宽字符串
#[cfg(windows)]
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
pub fn relaunch_as_admin() -> bool {
    false
}
