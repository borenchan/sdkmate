// 中国大陆时区检测（init 自动启用 GitHub 下载镜像的判据）
//
// 设计约束：
// - 纯尽力而为：任何读取失败/解析失败一律返回 false（= 不开镜像 = 维持现状），
//   绝不 panic、绝不影响 init 主流程
// - 按时区判断而非语言/IP：大陆统一为 UTC+8 无夏令时，IANA 时区名固定；
//   新加坡/吉隆坡同为 +8 但不命中，避免误开

use std::{env, fs};

/// 中国大陆的 IANA 时区别名（含历史别名 Chongqing/Harbin/Urumqi，老系统仍可能使用）
#[cfg(unix)]
const CHINA_TZ_MARKERS: &[&str] = &["Shanghai", "Chongqing", "Harbin", "Urumqi"];

/// Windows 时区的中国大陆标准名（大陆唯一时区，无夏令时）
#[cfg(windows)]
const WINDOWS_CHINA_TZ: &str = "China Standard Time";

/// 当前系统时区是否为中国大陆时区
///
/// Windows：读注册表 HKLM\SYSTEM\CurrentControlSet\Control\TimeZoneInformation 的 TimeZoneKeyName；
/// Unix：依次读 $TZ 环境变量 → /etc/timezone → /etc/localtime 符号链接目标。
/// 全部读取失败（容器/嵌入式等）返回 false。
pub fn is_china_mainland_timezone() -> bool {
    #[cfg(windows)]
    {
        is_windows_china_timezone()
    }
    #[cfg(unix)]
    {
        is_unix_china_timezone()
    }
}

/// Windows：TimeZoneKeyName 精确匹配 "China Standard Time"
/// （数据源为 HKLM\SYSTEM\CurrentControlSet\Control\TimeZoneInformation，tzutil /g 同源）
#[cfg(windows)]
fn is_windows_china_timezone() -> bool {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    hklm.open_subkey("SYSTEM\\CurrentControlSet\\Control\\TimeZoneInformation")
        .ok()
        .and_then(|key| key.get_value::<String, _>("TimeZoneKeyName").ok())
        .is_some_and(|name| name == WINDOWS_CHINA_TZ)
}

/// Unix：$TZ → /etc/timezone → /etc/localtime 链接目标，任一命中大陆时区别名
#[cfg(unix)]
fn is_unix_china_timezone() -> bool {
    // $TZ 环境变量（可能带路径前缀，如 ":Asia/Shanghai"）
    if let Ok(tz) = env::var("TZ") {
        if matches_china_tz(&tz) {
            return true;
        }
    }
    // /etc/timezone（Debian/Ubuntu 系）
    if let Ok(content) = fs::read_to_string("/etc/timezone") {
        if matches_china_tz(content.trim()) {
            return true;
        }
    }
    // /etc/localtime 符号链接目标（RHEL/Arch 系，形如 ../usr/share/zoneinfo/Asia/Shanghai）
    if let Ok(target) = fs::read_link("/etc/localtime") {
        if matches_china_tz(&target.to_string_lossy()) {
            return true;
        }
    }
    false
}

/// 字符串是否包含任一大陆时区别名（大小写不敏感；TZ 值可能带 ":"
/// 前缀或路径段，用子串匹配覆盖 "Asia/Shanghai" 与 "..zoneinfo/Asia/Shanghai"）
#[cfg(unix)]
fn matches_china_tz(s: &str) -> bool {
    let lower = s.to_lowercase();
    CHINA_TZ_MARKERS.iter().any(|m| lower.contains(&m.to_lowercase()))
}
