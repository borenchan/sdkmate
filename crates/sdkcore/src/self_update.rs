// 自更新：检查 GitHub 最新 release，下载对应平台产物，就地替换 sdkm 二进制。
//
// 关键设计（参考 rustup self_update，针对单二进制 CLI 折中）：
// - 平台检测用编译期 cfg!（= 已发布二进制的 target，非开发机；同 target 产物必能在同 target 二进制环境跑，
//   与 rustup macOS 用 from_build() 一致）。无产物的 target 编译期即 bail。
// - 替换前权限预检（仿 rustup self_update_permitted）：在 exe 父目录试写，碰网络前挡权限失败。
// - 全程 rename 不 delete：Windows 允许 rename 正在运行的 exe（MoveFileEx 改目录条目），
//   但不允许 delete/写打开 running exe——self_uninstall 注释的"running exe 自删不可靠"正指 delete，本方案避开。
// - 替换后 spawn `--version` 验证；失败自动回滚 .bak。--check 只查不下载，--rollback 恢复 .bak（本地不联网）。
// - 只升不降：远程 <= 当前 → 提示已是最新退出。
//
// 复用 install::downloader（reqwest 客户端 + 断点续传下载）与 install::extractor（zip/tar.gz 解压），
// 不重复造轮子；不新增依赖/配置；不碰 install/switch/list 等模块。

use crate::install::downloader::{build_reqwest_client, download_with_retry};
use crate::install::extractor::extract_archive;
use crate::manager::SdkManager;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use util::consts::SDKM_TMP_DIR;
use util::path::get_sdkm_home;
use util::{detail, info, success};

/// GitHub releases/latest API（拿 tag_name + assets 下载 URL，单请求）
const RELEASES_API: &str = "https://api.github.com/repos/borenchan/sdkmate/releases/latest";

/// 当前二进制版本（编译期烤入，与 `--version` 一致）
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct LatestRelease {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

impl SdkManager {
    /// 自更新入口（同步，内部 tokio runtime 驱动异步流程，与 install_sdk 范式一致）
    pub fn update_self(&self, check: bool, rollback: bool) -> Result<()> {
        // --rollback 是纯本地操作，不联网、不需 runtime
        if rollback {
            return do_rollback();
        }
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        rt.block_on(self.update_self_async(check))
    }

    /// 异步自更新核心：查询 → 比较 →（--check 到此为止）下载解压 → 替换验证
    async fn update_self_async(&self, check: bool) -> Result<()> {
        let client = build_reqwest_client(&self.config.network)?;

        // ── 查询 GitHub 最新 release ──
        info!("checking latest release from GitHub...");
        let resp = client
            .get(RELEASES_API)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .context("Failed to query GitHub releases API")?;
        let status = resp.status();
        if !status.is_success() {
            bail!(
                "GitHub API returned HTTP {} (rate limited? set github_token in config.toml)",
                status
            );
        }
        let release: LatestRelease = resp.json().await.context("Failed to parse latest release JSON")?;
        let latest_version = strip_v(&release.tag_name).to_string();

        detail!("current: v{}", CURRENT_VERSION);
        detail!("latest:  v{}", latest_version);

        // 只升不降：远程不更新于当前 → 已是最新
        if !is_newer(&latest_version, CURRENT_VERSION) {
            success!("sdkm is already up to date (v{})", CURRENT_VERSION);
            return Ok(());
        }

        if check {
            info!("update available: v{} → v{}", CURRENT_VERSION, latest_version);
            info!("run `sdkm self update` (without --check) to apply");
            return Ok(());
        }

        // ── 匹配平台产物 ──
        let asset_name = platform_asset_name()?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .with_context(|| format!("asset `{}` not found in latest release", asset_name))?;
        let download_url = asset.browser_download_url.clone();

        // ── 权限预检：在 exe 父目录试写（碰网络前挡权限失败）──
        let exe = env::current_exe().context("cannot locate sdkm executable")?;
        let exe_dir = exe.parent().context("exe has no parent directory")?;
        check_writable(exe_dir)?;

        // ── 下载 ──
        let home = get_sdkm_home()?;
        let work_dir = home.join(SDKM_TMP_DIR).join("self_update");
        fs::create_dir_all(&work_dir).context("Failed to create self_update work directory")?;
        let archive_path = work_dir.join(asset_name);

        info!("downloading {}...", asset_name);
        let pb = download_progress("Downloading sdkm...");
        download_with_retry(&client, &download_url, &archive_path, &pb, 3).await?;
        pb.finish_with_message("download complete");

        // ── 解压取包内 .sdkm/sdkm[.exe] ──
        let extract_dir = work_dir.join("extract");
        if extract_dir.exists() {
            let _ = fs::remove_dir_all(&extract_dir);
        }
        let ex_pb = spinner_progress("Extracting...");
        extract_archive(&archive_path, &extract_dir, &ex_pb)?;
        ex_pb.finish_with_message("extraction complete");

        let new_exe = extract_dir.join(".sdkm").join(current_bin_name());
        if !new_exe.exists() {
            bail!("extracted binary not found at {}", new_exe.display());
        }

        // ── 备份 + 替换 + 验证（失败自动回滚）──
        replace_binary(&exe, &new_exe, &latest_version)?;

        // 清理下载/解压临时文件（best-effort）；保留 work_dir 下的 .bak 供 --rollback
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&extract_dir);

        success!("sdkm updated to v{}", latest_version);
        info!("previous version backed up; run `sdkm self update --rollback` to restore it");
        Ok(())
    }
}

// ───────────────────────────── 替换 / 回滚 ─────────────────────────────

/// 备份当前 exe → 换入新二进制 → 验证 → 失败自动回滚
///
/// 备份与临时副本都放 work_dir（<home>/.tmp/self_update），exe 同目录保持干净。
fn replace_binary(exe: &Path, new_exe: &Path, target_version: &str) -> Result<()> {
    let work = work_dir()?;
    fs::create_dir_all(&work).context("Failed to create self_update work directory")?;
    let name = bin_name(exe);
    let bak = work.join(format!("{}.bak", name));

    // 清残留临时副本（.discard/.bad：上次 Windows 上删不掉的 running exe 副本）+ 旧 .bak
    clean_leftovers(&work, &name);
    if bak.exists() {
        fs::remove_file(&bak).context("Failed to remove stale backup")?;
    }

    // 备份当前 exe → work/.bak（rename running exe，同卷合法；不用 delete 避锁坑）
    fs::rename(exe, &bak).context(
        "Failed to back up current binary (SDKM_HOME on a different drive than sdkm.exe? \
         keep them on the same drive; or close other sdkm processes / check dir perms)",
    )?;

    // 换入新二进制（exe 路径已空，纯改名）
    if let Err(e) = fs::rename(new_exe, exe) {
        let _ = fs::rename(&bak, exe);
        bail!("Failed to place new binary: {}; rolled back to previous", e);
    }

    // 验证：spawn 新 exe --version，输出含目标版本号
    match spawn_version(exe) {
        Ok(out) if out.contains(target_version) => Ok(()),
        Ok(out) => {
            rollback_to_bak(exe, &bak, &work, &name);
            bail!(
                "new binary reported `{}` (expected v{}); rolled back to previous",
                out.trim(),
                target_version
            );
        }
        Err(e) => {
            rollback_to_bak(exe, &bak, &work, &name);
            bail!("new binary failed to start ({}); rolled back to previous", e);
        }
    }
}

/// --rollback：把 work/.bak 恢复为当前 exe（本地，不联网）
///
/// 备份不存在 → 不允许回滚（bail）。
fn do_rollback() -> Result<()> {
    let exe = env::current_exe().context("cannot locate sdkm executable")?;
    let work = work_dir()?;
    fs::create_dir_all(&work).context("Failed to create self_update work directory")?;
    let name = bin_name(&exe);
    // 清上次残留的临时副本（进程已退出，本次可删；.bak 保留）
    clean_leftovers(&work, &name);

    let bak = work.join(format!("{}.bak", name));
    if !bak.exists() {
        bail!("no backup to roll back to; run `sdkm self update` first to create one");
    }

    // 当前 exe 运行中 → rename 挪到 work/.discard 腾位（非 delete），再换入 .bak
    let discard = work.join(format!("{}.discard", name));
    fs::rename(&exe, &discard)
        .context("Failed to set aside current binary (SDKM_HOME on a different drive than sdkm.exe?)")?;
    if let Err(e) = fs::rename(&bak, &exe) {
        let _ = fs::rename(&discard, &exe); // 换入失败，恢复原 exe
        bail!("Failed to restore backup: {}", e);
    }

    // 验证回滚后的二进制能启动
    match spawn_version(&exe) {
        Ok(out) => {
            // discard 是刚被 rename 走的原 running exe 副本，Windows 上进程仍占用可能删不掉；
            // 删不掉则留在 work_dir（不污染 exe 目录），下次 self update / --rollback 开头 clean_leftovers 清理
            let _ = fs::remove_file(&discard);
            success!("rolled back to previous version ({})", out.trim());
            Ok(())
        }
        Err(e) => {
            // .bak 损坏：把 discard（原好 exe）恢复回来
            let bad = work.join(format!("{}.bad", name));
            let _ = fs::rename(&exe, &bad);
            let _ = fs::rename(&discard, &exe);
            let _ = fs::remove_file(&bad);
            bail!("restored backup failed verification ({}); original binary restored", e);
        }
    }
}

/// 备份回滚：exe 当前是坏的新二进制，恢复 .bak 到 exe（临时副本放 work_dir）
fn rollback_to_bak(exe: &Path, bak: &Path, work: &Path, name: &str) {
    let discard = work.join(format!("{}.discard", name));
    let _ = fs::rename(exe, &discard); // 坏的新二进制挪到 work
    let _ = fs::rename(bak, exe); // .bak 恢复
    let _ = fs::remove_file(&discard);
}

// ───────────────────────────── 工具函数 ─────────────────────────────

/// 当前平台的 release asset 名（编译期确定，与 CI 矩阵一致）
fn platform_asset_name() -> Result<&'static str> {
    let name = if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "sdkm-windows-x86_64.zip"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "sdkm-macos-aarch64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "sdkm-macos-x86_64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") && cfg!(target_env = "musl") {
        "sdkm-linux-x86_64-musl.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") && cfg!(target_env = "gnu") {
        "sdkm-linux-x86_64-gnu.tar.gz"
    } else {
        bail!(
            "no prebuilt sdkm asset for this platform; download manually from \
             https://github.com/borenchan/sdkmate/releases"
        );
    };
    Ok(name)
}

/// 当前平台的二进制文件名（包内 .sdkm/ 下）
fn current_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "sdkm.exe"
    } else {
        "sdkm"
    }
}

/// self_update 工作目录（<home>/.tmp/self_update）：存放备份/下载/解压/临时副本，
/// 使 exe 同目录保持干净（备份与 .discard 残留都在此，不污染 sdkm 安装目录）
fn work_dir() -> Result<PathBuf> {
    let home = get_sdkm_home()?;
    Ok(home.join(SDKM_TMP_DIR).join("self_update"))
}

/// exe 的文件名（用于在 work_dir 下构造备份/临时副本名）
fn bin_name(exe: &Path) -> String {
    exe.file_name().and_then(|n| n.to_str()).unwrap_or("sdkm").to_string()
}

/// 清理 work_dir 下残留的临时副本（.discard / .bad）：上次 rollback 在 Windows 上因进程占用删不掉的
/// running exe 副本。best-effort：删不掉则忽略（进程退出后下次调用可删）。不碰 .bak（有效备份）。
fn clean_leftovers(work: &Path, name: &str) {
    for ext in ["discard", "bad"] {
        let p = work.join(format!("{}.{}", name, ext));
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
    }
}

/// 权限预检：在 exe 父目录建临时文件试写，PermissionDenied → bail（碰网络前挡）
fn check_writable(dir: &Path) -> Result<()> {
    let test = dir.join(".sdkm_write_test");
    match fs::File::create(&test) {
        Ok(_) => {
            let _ = fs::remove_file(&test);
            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            bail!(
                "no write permission to `{}`; run as admin/sudo or update manually from \
                 https://github.com/borenchan/sdkmate/releases",
                dir.display()
            );
        }
        Err(e) => {
            let _ = fs::remove_file(&test);
            bail!("cannot write to `{}`: {}", dir.display(), e)
        }
    }
}

/// spawn exe --version，返回 stdout（验证二进制可启动 + 版本号）
fn spawn_version(exe: &Path) -> Result<String> {
    let out = Command::new(exe)
        .arg("--version")
        .output()
        .with_context(|| format!("Failed to spawn `{}` --version", exe.display()))?;
    if !out.status.success() {
        bail!("`{} --version` exited with {}", exe.display(), out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// tag 去掉前导 'v'（v0.2.8 → 0.2.8）
fn strip_v(tag: &str) -> &str {
    tag.trim().trim_start_matches('v')
}

/// 解析 "x.y.z" 为三段 u64（无法解析的段当 0）
fn parse_ver(s: &str) -> (u64, u64, u64) {
    let mut parts = s.split('.');
    let major = parts.next().map(first_digits).unwrap_or(0);
    let minor = parts.next().map(first_digits).unwrap_or(0);
    let patch = parts.next().map(first_digits).unwrap_or(0);
    (major, minor, patch)
}

/// 取段开头连续数字（"0-rc1" → 0，"2beta" → 2）
fn first_digits(seg: &str) -> u64 {
    let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

/// latest 是否更新于 current（只升不降）
fn is_newer(latest: &str, current: &str) -> bool {
    parse_ver(latest) > parse_ver(current)
}

/// 下载进度条（bytes bar）
fn download_progress(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template("  {msg}\n  {bar:40.green/blue} {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap()
            .progress_chars("▓▒░"),
    );
    pb.set_message(msg.to_owned());
    pb.enable_steady_tick(Duration::from_millis(500));
    pb
}

/// 解压 spinner
fn spinner_progress(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_owned());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}
