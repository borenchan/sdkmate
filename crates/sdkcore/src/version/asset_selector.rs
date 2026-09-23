// GitHub Releases 平台资产选择器（纯函数，无 IO 无状态）
//
// 从一个 release 的资产列表中挑选当前平台的压缩包。规则为行业惯例收敛的
// 同义词表 + 打分制，零配置：绝大多数 GH 分发工具无需任何额外说明即可命中。
//
// 选择流程（两段门）：
//   1. 后缀白名单（.zip / .tar.gz / .tgz）——天然排除 .sha256/.bsdiff/.msi/.deb/.dmg/.asc 签名类与裸二进制
//   2. 前缀门：资产主名必须以 `{asset_prefix}-` / `{asset_prefix}_` 开头（如 bun-/gh_），
//      剥离前缀后剩余每段都必须可识别（OS/arch/桥接/修饰词/libc/版本数字）——
//      未识别段直接拒绝（排除同 release 内兄弟工具资产，如 codex 的 app-server、uv 的 uvx）
//   3. 多候选打分：修饰词(baseline/profile/debug)降权 > libc(musl/gnu) 与编译目标不匹配降权 > 平分取先出现

/// 压缩包后缀白名单（解压器支持的格式）
const EXTENSIONS: &[&str] = &[".zip", ".tar.gz", ".tgz"];

/// 修饰词：同一平台的非默认变体（性能基线 / 性能分析 / 调试），一律降权
const MODIFIER_TOKENS: &[&str] = &["baseline", "profile", "debug"];

/// libc 变体段（linux 专属，参与打分不参与拒绝）
const MUSL_TOKEN: &str = "musl";
const GNU_TOKEN: &str = "gnu";

/// 平台桥接段：出现在平台三元组（x86_64-pc-windows-msvc 等）或双后缀（.exe.zip）中
const BRIDGE_TOKENS: &[&str] = &["pc", "unknown", "apple", "msvc", "exe"];

/// 当前编译目标的 OS / ARCH 标准名（与同义词表比对用）
const HOST_OS: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "darwin"
} else {
    "linux"
};

const HOST_ARCH: &str = if cfg!(target_arch = "x86_64") {
    "x64"
} else if cfg!(target_arch = "aarch64") {
    "arm64"
} else {
    "x86"
};

/// 本机是否 musl 链接（musl 版 sdkm 跑在 Alpine 等，应优先选 musl 资产）
const HOST_IS_MUSL: bool = cfg!(target_env = "musl");

/// 资产候选（GH releases API 的 assets 条目子集）
#[derive(Debug, Clone, Copy)]
pub struct AssetCandidate<'a> {
    pub name: &'a str,
    pub download_url: &'a str,
}

/// 从资产列表中选出当前平台的最优资产；无匹配返回 None（调用方回落模板下载）
///
/// `asset_prefix`：工具在资产名中的前缀（不含分隔符，如 "bun"/"claude"/"gh"）
pub fn pick_asset<'a>(candidates: &[AssetCandidate<'a>], asset_prefix: &str) -> Option<AssetCandidate<'a>> {
    let mut scored: Vec<(i32, usize)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, c)| score_asset(c.name, asset_prefix).map(|s| (s, idx)))
        .collect();
    if scored.is_empty() {
        return None;
    }
    // 稳定排序：分数高者优先，平分保持原始出现顺序
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let best = candidates[scored[0].1];
    Some(AssetCandidate {
        name: best.name,
        download_url: best.download_url,
    })
}

/// 对单个资产名打分：不匹配返回 None；匹配返回分数（越高越好）
fn score_asset(name: &str, asset_prefix: &str) -> Option<i32> {
    // 0. 预处理：x86_64 的下划线是词内字符而非分隔符，归一化成 x64 再切段
    let name = name.to_lowercase().replace("x86_64", "x64");

    // 1. 后缀白名单（大小写不敏感）
    let lower = name;
    if !EXTENSIONS.iter().any(|e| lower.ends_with(e)) {
        return None;
    }

    // 2. 前缀门：剥离 "{prefix}-" / "{prefix}_"，失败则拒绝
    let stem = lower
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz")
        .trim_end_matches(".zip");
    let stem = stem
        .strip_prefix(&format!("{}-", asset_prefix))
        .or_else(|| stem.strip_prefix(&format!("{}_", asset_prefix)))?;

    // 3. 切段分类：每段必须可识别，未识别段拒绝（过滤兄弟工具资产）
    let tokens: Vec<&str> = stem.split(['-', '_', '.']).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return None;
    }

    let mut os_hit = false;
    let mut arch_hit = false;
    let mut score = 0;
    for token in &tokens {
        if let Some(os) = os_synonym(token) {
            if os != HOST_OS {
                return None; // 明确是其他平台的资产
            }
            os_hit = true;
        } else if let Some(arch) = arch_synonym(token) {
            if arch != HOST_ARCH {
                return None; // 明确是其他架构的资产
            }
            arch_hit = true;
        } else if BRIDGE_TOKENS.contains(token) {
            // 平台三元组桥接段 / 双后缀 exe 段
        } else if *token == "universal" {
            arch_hit = true; // 无 arch 细分的通用包
        } else if *token == MUSL_TOKEN || *token == GNU_TOKEN {
            // libc 段：不影响命中，由打分决定
        } else if is_version_token(token) {
            // 版本数字段（cmake-4.3.5-…、helm-v4.2.4-…）
        } else if MODIFIER_TOKENS.contains(token) {
            score -= 4; // baseline/profile/debug 修饰变体降权
        } else {
            return None; // 未识别段：拒绝（app-server/bwrap 等异名资产在此被排除）
        }
    }

    if !os_hit || !arch_hit {
        return None; // 必须同时命中当前平台的 OS 和 arch
    }

    // 4. libc 偏好：musl 版 sdkm 优先 musl 资产，反之优先 gnu/默认
    let is_musl = tokens.contains(&MUSL_TOKEN);
    let is_gnu = tokens.contains(&GNU_TOKEN);
    if is_musl && !HOST_IS_MUSL {
        score -= 3;
    }
    if is_gnu && HOST_IS_MUSL {
        score -= 3;
    }
    Some(score)
}

/// 版本数字段：纯数字（"4"/"100"）或 v+数字（"v4"）
fn is_version_token(token: &str) -> bool {
    let digits = token.strip_prefix('v').unwrap_or(token);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// OS 同义词表 → 标准化 OS 名（windows/darwin/linux）
/// 剥掉尾随数字以兼容 macos10.10 这类带版本的段
fn os_synonym(token: &str) -> Option<&'static str> {
    let trimmed = token.trim_end_matches(|c: char| c.is_ascii_digit());
    match trimmed {
        "windows" | "win32" | "win" => Some("windows"),
        "darwin" | "macos" | "mac" | "osx" => Some("darwin"),
        "linux" => Some("linux"),
        _ => None,
    }
}

/// arch 同义词表 → 标准化 arch 名（x64/arm64/x86）
fn arch_synonym(token: &str) -> Option<&'static str> {
    match token {
        "x64" | "x86_64" | "amd64" => Some("x64"),
        "arm64" | "aarch64" => Some("arm64"),
        "x86" | "386" | "i686" | "i386" => Some("x86"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c<'a>(name: &'a str) -> AssetCandidate<'a> {
        AssetCandidate {
            name,
            download_url: "https://example.com",
        }
    }

    fn url_of(pick: Option<AssetCandidate>) -> String {
        pick.unwrap().download_url.to_string()
    }

    // ── bun：修饰变体降权 + gnu/musl 偏好 ─────────────────────────

    #[test]
    fn bun_windows_prefers_plain_over_baseline() {
        let assets = [
            c("bun-windows-x64-baseline.zip"),
            c("bun-windows-x64.zip"),
            c("bun-windows-x64-profile.zip"),
        ];
        let picked = pick_asset(&assets, "bun").unwrap();
        assert_eq!(picked.name, "bun-windows-x64.zip");
    }

    #[test]
    fn bun_linux_prefers_gnu_over_musl() {
        // gnu/musl 偏好是 linux 平台语义：在 linux 宿主选 gnu，非 linux 宿主不命中
        let assets = [c("bun-linux-x64-musl.zip"), c("bun-linux-x64.zip")];
        let picked = pick_asset(&assets, "bun");
        if cfg!(target_os = "linux") {
            assert_eq!(picked.unwrap().name, "bun-linux-x64.zip");
        } else {
            assert!(picked.is_none());
        }
    }

    #[test]
    fn bun_mac_arm64_only_hits_on_mac() {
        let assets = [c("bun-darwin-aarch64.zip"), c("bun-darwin-x64.zip")];
        let picked = pick_asset(&assets, "bun");
        if cfg!(target_os = "macos") {
            assert_eq!(picked.unwrap().name, "bun-darwin-aarch64.zip");
        } else {
            assert!(picked.is_none());
        }
    }

    // ── codex：前缀门排除兄弟工具资产 + exe.zip 双后缀 ────────────

    #[test]
    fn codex_picks_main_binary_excluding_app_server_and_sibling_tools() {
        let assets = [
            c("bwrap-x86_64-unknown-linux-musl.tar.gz"),
            c("codex-app-server-x86_64-pc-windows-msvc.exe.zip"),
            c("codex-x86_64-pc-windows-msvc.exe.zip"),
            c("codex-x86_64-pc-windows-msvc.exe.tar.gz"),
        ];
        let picked = pick_asset(&assets, "codex").unwrap();
        assert_eq!(picked.name, "codex-x86_64-pc-windows-msvc.exe.zip");
    }

    // ── uv：前缀门排除 uvx ─────────────────────────────────────────

    #[test]
    fn uv_excludes_uvx_assets() {
        let assets = [c("uvx-x86_64-pc-windows-msvc.zip"), c("uv-x86_64-pc-windows-msvc.zip")];
        let picked = pick_asset(&assets, "uv").unwrap();
        assert_eq!(picked.name, "uv-x86_64-pc-windows-msvc.zip");
    }

    // ── claude-code：前缀与 sdk 名不同（claude vs claude-code）────

    #[test]
    fn claude_win32_naming() {
        let assets = [c("claude-win32-x64.zip"), c("SHASUMS256.txt")];
        let picked = pick_asset(&assets, "claude").unwrap();
        assert_eq!(picked.name, "claude-win32-x64.zip");
    }

    // ── pnpm / gh / deno：win32 词表、下划线分段、三元组 ──────────

    #[test]
    fn pnpm_win32_word() {
        let assets = [c("pnpm-win32-x64.zip"), c("pnpm-linux-x64.tar.gz")];
        let picked = pick_asset(&assets, "pnpm").unwrap();
        assert_eq!(picked.name, "pnpm-win32-x64.zip");
    }

    #[test]
    fn gh_underscore_naming_and_amd64() {
        let assets = [
            c("gh_2.100.0_windows_amd64.zip"),
            c("gh_2.100.0_linux_amd64.tar.gz"),
            c("gh_2.100.0_checksums.txt"),
        ];
        let picked = pick_asset(&assets, "gh").unwrap();
        assert_eq!(picked.name, "gh_2.100.0_windows_amd64.zip");
    }

    #[test]
    fn deno_platform_triple_with_bsdiff_excluded() {
        let assets = [
            c("deno-x86_64-pc-windows-msvc.from-2.9.5.bsdiff"),
            c("deno-x86_64-pc-windows-msvc.zip"),
            c("deno-x86_64-pc-windows-msvc.sha256sum"),
        ];
        let picked = pick_asset(&assets, "deno").unwrap();
        assert_eq!(picked.name, "deno-x86_64-pc-windows-msvc.zip");
    }

    // ── cmake：版本号段 + macos universal ──────────────────────────

    #[test]
    fn cmake_windows_with_version_tokens() {
        let assets = [
            c("cmake-4.3.5-files-v1.json"),
            c("cmake-4.3.5-windows-x86_64.zip"),
            c("cmake-4.3.5-windows-x86_64.msi"),
        ];
        let picked = pick_asset(&assets, "cmake").unwrap();
        assert_eq!(picked.name, "cmake-4.3.5-windows-x86_64.zip");
    }

    #[test]
    fn cmake_macos_universal_matches_any_arch() {
        let assets = [
            c("cmake-4.3.5-macos-universal.tar.gz"),
            c("cmake-4.3.5-macos10.10-universal.tar.gz"),
        ];
        let picked = pick_asset(&assets, "cmake");
        if cfg!(target_os = "macos") {
            // 精确词 macos 优先于带尾数字的 macos10.10
            assert_eq!(picked.unwrap().name, "cmake-4.3.5-macos-universal.tar.gz");
        } else {
            assert!(picked.is_none());
        }
    }

    // ── terraform：资产缺失回落 None ───────────────────────────────

    #[test]
    fn terraform_assetless_release_returns_none() {
        let assets = [c("terraform_1.16.1_SHA256SUMS")];
        assert!(pick_asset(&assets, "terraform").is_none());
    }

    // ── 平台排斥：其他平台资产不命中 ───────────────────────────────

    #[test]
    fn other_platform_assets_rejected() {
        let assets = [c("bun-linux-x64.zip"), c("bun-darwin-aarch64.zip")];
        let picked = pick_asset(&assets, "bun");
        if cfg!(target_os = "windows") {
            assert!(picked.is_none());
        }
    }
}
