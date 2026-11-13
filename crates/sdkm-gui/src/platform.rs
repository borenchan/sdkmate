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
    use std::os::windows::ffi::OsStrExt;
    /// 转为以 NUL 结尾的宽字符串
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

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

#[cfg(not(windows))]
pub fn relaunch_as_admin() -> bool {
    false
}
