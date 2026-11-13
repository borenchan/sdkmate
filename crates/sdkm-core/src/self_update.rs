// 自更新：检查 GitHub release，下载对应平台产物，替换 CLI 与 GUI 二进制。
//
// 关键设计：
// - CLI 与 GUI 版本必须对齐：同目录存在 sdkm-gui 时使用 desktop 组合包，一次替换两个二进制；
//   纯 CLI 安装继续使用单 CLI 包；目标版本缺少 desktop 包时直接报错，绝不产生版本不一致。
// - 回退不再使用本地 .bak：查询发布列表，选择低于当前版本且包含所需组合包的最新版本，
//   走与升级完全相同的下载/替换流程。
// - 替换前权限预检；全程 rename 不 delete（Windows 允许 rename 正在运行的 exe）。
// - 替换后逐个 spawn `--version` 验证；失败恢复原文件。

use crate::install::downloader::{build_download_client, download_with_retry};
use crate::install::extractor::extract_archive;
use crate::manager::SdkManager;
use crate::report::ProgressKind;
use anyhow::{Context, Result, bail};
use sdkm_util::consts::SDKM_TMP_DIR;
use sdkm_util::path::get_sdkm_home;
use serde::Deserialize;
use std::cmp::Reverse;
use std::env;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// GitHub releases/latest API（拿 tag_name + assets 下载 URL，单请求）
const RELEASES_API: &str = "https://api.github.com/repos/borenchan/sdkmate/releases/latest";

/// GitHub releases 列表 API（回退时挑选上一个可用版本；100 条足够覆盖历史）
const RELEASES_LIST_API: &str = "https://api.github.com/repos/borenchan/sdkmate/releases?per_page=100";

/// 当前二进制版本（编译期烤入，与 `--version` 一致）
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 探测二进制版本的最长等待时间（旧版 GUI 不认识 --version 会打开窗口，必须兜底 kill）
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Deserialize, Clone)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
    /// 草稿/预发布不参与回退（与 releases/latest 的默认语义一致）
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Deserialize, Clone)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// 需要替换的二进制组合
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bundle {
    /// 只有 CLI 的纯命令行安装
    Cli,
    /// CLI + GUI 同目录的桌面组合
    Desktop,
}

/// 自更新结果（CLI 忽略返回值；GUI 据此决定是否提示重启）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfUpdateOutcome {
    /// 升级检查：所有二进制都已是目标版本
    UpToDate { version: String },
    /// 仅检查：存在可用更新
    Checked { latest: String },
    /// 已替换到新版本
    Updated { version: String },
    /// 已回退到旧版本
    RolledBack { version: String },
}

/// 同目录下参与版本对齐的二进制
struct InstalledBinaries {
    cli: PathBuf,
    gui: Option<PathBuf>,
}

impl InstalledBinaries {
    fn bundle(&self) -> Bundle {
        if self.gui.is_some() {
            Bundle::Desktop
        } else {
            Bundle::Cli
        }
    }
}

impl SdkManager {
    /// 自更新入口（同步，内部 tokio runtime 驱动异步流程，与 install_sdk 范式一致）
    pub fn update_self(&self, check: bool, rollback: bool) -> Result<SelfUpdateOutcome> {
        if check && rollback {
            bail!("`--check` and `--rollback` are mutually exclusive");
        }
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        rt.block_on(self.update_self_async(check, rollback))
    }

    /// 查询 GitHub 最新版本号（只读：不下载不替换；供检查更新/GUI 复用）
    pub fn fetch_latest_version(&self) -> Result<String> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        rt.block_on(async {
            let release = self.fetch_latest_release().await?;
            Ok(strip_v(&release.tag_name).to_string())
        })
    }

    /// 回退目标：低于当前版本、且包含所需组合包的最新发布版本（只读，供 GUI 确认弹窗展示）
    pub fn fetch_previous_version(&self) -> Result<String> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        rt.block_on(async {
            let release = self.fetch_previous_release().await?;
            Ok(strip_v(&release.tag_name).to_string())
        })
    }

    /// 拉取 GitHub 最新 release（token 只注入本请求，避免下载时把 token 发给镜像第三方）
    async fn fetch_latest_release(&self) -> Result<Release> {
        self.sink.info("checking latest release from GitHub...".to_string());
        let client = build_download_client(&self.config.network)?;
        let mut req = client.get(RELEASES_API).header("Accept", "application/vnd.github+json");
        if let Some(token) = &self.config.network.github_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await.context("Failed to query GitHub releases API")?;
        let status = resp.status();
        if !status.is_success() {
            bail!(
                "GitHub API returned HTTP {} (rate limited? set github_token in config.toml)",
                status
            );
        }
        resp.json().await.context("Failed to parse latest release JSON")
    }

    /// 拉取发布列表（回退挑选目标版本用）
    async fn fetch_releases(&self) -> Result<Vec<Release>> {
        self.sink.info("querying GitHub releases...".to_string());
        let client = build_download_client(&self.config.network)?;
        let mut req = client.get(RELEASES_LIST_API).header("Accept", "application/vnd.github+json");
        if let Some(token) = &self.config.network.github_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await.context("Failed to query GitHub releases API")?;
        let status = resp.status();
        if !status.is_success() {
            bail!(
                "GitHub API returned HTTP {} (rate limited? set github_token in config.toml)",
                status
            );
        }
        resp.json().await.context("Failed to parse releases JSON")
    }

    /// 挑选回退目标：版本低于当前、含所需组合包、版本号最大
    async fn fetch_previous_release(&self) -> Result<Release> {
        let bundle = installed_binaries()?.bundle();
        let wanted_asset = asset_name(bundle)?;
        let mut older: Vec<Release> = self
            .fetch_releases()
            .await?
            .into_iter()
            .filter(|r| !r.draft && !r.prerelease)
            .filter(|r| is_newer(CURRENT_VERSION, strip_v(&r.tag_name)))
            .filter(|r| r.assets.iter().any(|a| a.name == wanted_asset))
            .collect();
        // 高版本在前
        older.sort_by_key(|r| Reverse(parse_ver(strip_v(&r.tag_name))));
        older.into_iter().next().with_context(|| {
            format!(
                "no older release contains `{}`; cannot roll back without breaking CLI/GUI version alignment",
                wanted_asset
            )
        })
    }

    /// 异步自更新核心：查询 → 对齐检查 →（--check 到此为止）下载解压 → 替换验证
    async fn update_self_async(&self, check: bool, rollback: bool) -> Result<SelfUpdateOutcome> {
        let release = if rollback {
            self.fetch_previous_release().await?
        } else {
            self.fetch_latest_release().await?
        };
        let target_version = strip_v(&release.tag_name).to_string();

        self.sink.detail(format!("current: v{}", CURRENT_VERSION));
        self.sink.detail(format!("target:  v{}", target_version));

        let binaries = installed_binaries()?;

        // 升级：CLI/GUI 都已对齐到最新 → 无操作；--check 只报告
        if !rollback && all_at_version(&binaries, &target_version) {
            self.sink.success(format!("sdkm is already up to date (v{})", target_version));
            return Ok(SelfUpdateOutcome::UpToDate {
                version: target_version,
            });
        }
        if check {
            self.sink
                .info(format!("update available: v{} -> v{}", CURRENT_VERSION, target_version));
            self.sink.info("run `sdkm self update` (without --check) to apply".to_string());
            return Ok(SelfUpdateOutcome::Checked { latest: target_version });
        }

        // ── 匹配平台产物（有 GUI 必须用 desktop 组合包）──
        let bundle = binaries.bundle();
        let asset_name = asset_name(bundle)?;
        let asset = release.assets.iter().find(|a| a.name == asset_name).with_context(|| {
            format!(
                "asset `{}` not found in release v{}; a CLI+GUI install must be updated together, \
                 so this version cannot be applied without breaking version alignment",
                asset_name, target_version
            )
        })?;
        // GitHub 下载镜像重写（白名单域名拼前缀；关闭态原样返回）
        let download_url = self.config.network.mirror_github_url(&asset.browser_download_url);

        // ── 权限预检：在 CLI 父目录试写（碰网络前挡权限失败）──
        let exe_dir = binaries.cli.parent().context("CLI binary has no parent directory")?;
        check_writable(exe_dir)?;

        // ── 下载 ──
        let home = get_sdkm_home()?;
        let work_dir = home.join(SDKM_TMP_DIR).join("self_update");
        fs::create_dir_all(&work_dir).context("Failed to create self_update work directory")?;
        let archive_path = work_dir.join(asset_name);

        self.sink.info(format!("downloading {}...", asset_name));
        let pb = self
            .sink
            .progress(ProgressKind::SelfUpdateDownload, "Downloading sdkm...".to_string());
        let client = build_download_client(&self.config.network)?;
        download_with_retry(&client, &download_url, &archive_path, pb.as_ref(), self.sink.as_ref(), 3).await?;
        pb.finish_with_message("download complete".to_string());

        // ── 解压取包内 .sdkm/ 下的二进制 ──
        let extract_dir = work_dir.join("extract");
        if extract_dir.exists() {
            let _ = fs::remove_dir_all(&extract_dir);
        }
        let ex_pb = self.sink.progress(ProgressKind::SelfUpdateExtract, "Extracting...".to_string());
        extract_archive(&archive_path, &extract_dir, ex_pb.as_ref())?;
        ex_pb.finish_with_message("extraction complete".to_string());

        let new_cli = extract_dir.join(".sdkm").join(cli_bin_name());
        if !new_cli.exists() {
            bail!("extracted CLI binary not found at {}", new_cli.display());
        }
        let new_gui = extract_dir.join(".sdkm").join(gui_bin_name());
        if binaries.gui.is_some() && !new_gui.exists() {
            bail!(
                "desktop bundle `{}` does not contain {}; refusing a CLI-only update",
                asset_name,
                gui_bin_name()
            );
        }

        // ── 替换 + 验证（失败恢复原文件）──
        let mut pairs = vec![(binaries.cli.clone(), new_cli)];
        if let Some(gui) = &binaries.gui {
            pairs.push((gui.clone(), new_gui));
        }
        swap_binaries(&pairs, &target_version)?;

        // 清理下载/解压临时文件（discard 残留交给下次 clean_leftovers）
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&extract_dir);

        if rollback {
            self.sink.success(format!("sdkm rolled back to v{}", target_version));
            Ok(SelfUpdateOutcome::RolledBack {
                version: target_version,
            })
        } else {
            self.sink.success(format!("sdkm updated to v{}", target_version));
            if binaries.gui.is_some() {
                self.sink.info("restart sdkm-gui to use the new version".to_string());
            }
            Ok(SelfUpdateOutcome::Updated {
                version: target_version,
            })
        }
    }
}

// ───────────────────────────── 替换 / 恢复 ─────────────────────────────

/// 待替换的一对：当前路径 + 新二进制路径 + 临时挪走的原文件
struct Staged {
    current: PathBuf,
    new_path: PathBuf,
    discard: PathBuf,
}

/// 一次替换全部二进制：先全部挪走旧文件，再全部换入新文件，最后逐个校验版本
///
/// 任何一步失败都会把已挪走的原文件恢复回来，避免 CLI/GUI 版本不一致。
fn swap_binaries(pairs: &[(PathBuf, PathBuf)], target_version: &str) -> Result<()> {
    let work = work_dir()?;
    fs::create_dir_all(&work).context("Failed to create self_update work directory")?;
    let mut staged: Vec<Staged> = Vec::with_capacity(pairs.len());

    // 阶段 1：旧二进制 rename 到临时区（不 delete，避开 Windows 运行中锁定）
    for (current, new_path) in pairs {
        let name = bin_name(current);
        clean_leftovers(&work, &name);
        // 带 PID 的临时名：同一进程重复更新时不会撞上上次残留（运行中的 GUI 副本可能删不掉）
        let discard = work.join(format!("{}.{}.discard", name, std::process::id()));
        if let Err(e) = fs::rename(current, &discard) {
            restore_staged(&staged);
            bail!(
                "Failed to set aside `{}`: {} (keep sdkm home on the same drive as the binaries; \
                 close other sdkm/sdkm-gui instances and retry)",
                current.display(),
                e
            );
        }
        staged.push(Staged {
            current: current.clone(),
            new_path: new_path.clone(),
            discard,
        });
    }

    // 阶段 2：新二进制换入
    for s in &staged {
        if let Err(e) = fs::rename(&s.new_path, &s.current) {
            restore_staged(&staged);
            bail!(
                "Failed to place new binary `{}`: {}; original binaries restored",
                s.current.display(),
                e
            );
        }
    }

    // 验证：逐个执行 --version，确认版本与目标一致
    for s in &staged {
        match spawn_version(&s.current) {
            Ok(out) if reports_version(&out, target_version) => {}
            Ok(out) => {
                restore_staged(&staged);
                bail!(
                    "new binary `{}` reported `{}` (expected v{}); original binaries restored",
                    s.current.display(),
                    out.trim(),
                    target_version
                );
            }
            Err(e) => {
                restore_staged(&staged);
                bail!(
                    "new binary `{}` failed to start ({}); original binaries restored",
                    s.current.display(),
                    e
                );
            }
        }
    }

    // 成功：清理临时副本；运行中的 GUI 副本可能删不掉，交给下次 clean_leftovers
    for s in &staged {
        let _ = fs::remove_file(&s.discard);
    }
    Ok(())
}

/// 替换失败时恢复：把已换入的新二进制移到 .bad，再把原文件换回
fn restore_staged(staged: &[Staged]) {
    for s in staged.iter().rev() {
        let bad = s.discard.with_extension("bad");
        if s.current.exists() {
            let _ = fs::remove_file(&bad);
            let _ = fs::rename(&s.current, &bad);
        }
        if s.discard.exists() {
            let _ = fs::rename(&s.discard, &s.current);
        }
        let _ = fs::remove_file(&bad);
    }
}

// ───────────────────────────── 工具函数 ─────────────────────────────

/// 当前进程同目录下参与版本对齐的二进制（CLI 必须存在；GUI 可选）
fn installed_binaries() -> Result<InstalledBinaries> {
    let exe = env::current_exe().context("cannot locate sdkm executable")?;
    let dir = exe.parent().context("sdkm executable has no parent directory")?;
    let cli = dir.join(cli_bin_name());
    if !cli.is_file() {
        bail!(
            "CLI binary `{}` not found in the same directory as the running executable; \
             keep sdkm and sdkm-gui together so their versions stay aligned",
            cli.display()
        );
    }
    let gui = dir.join(gui_bin_name());
    Ok(InstalledBinaries {
        cli,
        gui: gui.is_file().then_some(gui),
    })
}

/// 所有已安装二进制是否都报告目标版本（GUI 不支持 --version 的旧版本会被视为未对齐）
fn all_at_version(binaries: &InstalledBinaries, version: &str) -> bool {
    let cli_ok = spawn_version(&binaries.cli)
        .map(|out| reports_version(&out, version))
        .unwrap_or(false);
    let gui_ok = binaries
        .gui
        .as_ref()
        .is_none_or(|path| spawn_version(path).map(|out| reports_version(&out, version)).unwrap_or(false));
    cli_ok && gui_ok
}

/// 探测 CLI 版本（供诊断展示同目录 CLI 的版本；GUI 版本取运行中编译期常量）
pub(crate) fn sibling_cli_version() -> Option<String> {
    let exe = env::current_exe().ok()?;
    let cli = exe.parent()?.join(cli_bin_name());
    if !cli.is_file() {
        return None;
    }
    spawn_version(&cli)
        .ok()
        .and_then(|out| out.split_whitespace().last().map(|v| v.trim_start_matches('v').to_string()))
        .filter(|v| !v.is_empty())
}

/// 当前平台的 CLI 单包名（编译期确定，与 CI 矩阵一致）
fn cli_asset_name() -> Result<&'static str> {
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
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") && cfg!(target_env = "musl") {
        "sdkm-linux-aarch64-musl.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") && cfg!(target_env = "gnu") {
        "sdkm-linux-aarch64-gnu.tar.gz"
    } else {
        bail!(
            "no prebuilt sdkm asset for this platform; download manually from \
             https://github.com/borenchan/sdkmate/releases"
        );
    };
    Ok(name)
}

/// 当前平台的 Desktop 组合包名（CLI+GUI，仅 gnu Linux/macOS/Windows 提供）
fn desktop_asset_name() -> Result<&'static str> {
    let name = if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "sdkm-desktop-windows-x86_64.zip"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "sdkm-desktop-macos-aarch64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "sdkm-desktop-macos-x86_64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") && cfg!(target_env = "gnu") {
        "sdkm-desktop-linux-x86_64-gnu.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") && cfg!(target_env = "gnu") {
        "sdkm-desktop-linux-aarch64-gnu.tar.gz"
    } else {
        bail!(
            "no prebuilt sdkm desktop bundle for this platform; download manually from \
             https://github.com/borenchan/sdkmate/releases"
        );
    };
    Ok(name)
}

fn asset_name(bundle: Bundle) -> Result<&'static str> {
    match bundle {
        Bundle::Cli => cli_asset_name(),
        Bundle::Desktop => desktop_asset_name(),
    }
}

/// 当前平台的 CLI 二进制文件名（包内 .sdkm/ 下）
fn cli_bin_name() -> &'static str {
    if cfg!(windows) { "sdkm.exe" } else { "sdkm" }
}

/// 当前平台的 GUI 二进制文件名（包内 .sdkm/ 下）
fn gui_bin_name() -> &'static str {
    if cfg!(windows) { "sdkm-gui.exe" } else { "sdkm-gui" }
}

/// self_update 工作目录（<home>/.tmp/self_update）：存放下载/解压/临时副本，
/// 使 exe 同目录保持干净（临时副本残留都在此，不污染 sdkm 安装目录）
fn work_dir() -> Result<PathBuf> {
    let home = get_sdkm_home()?;
    Ok(home.join(SDKM_TMP_DIR).join("self_update"))
}

/// exe 的文件名（用于在 work_dir 下构造临时副本名）
fn bin_name(exe: &Path) -> String {
    exe.file_name().and_then(|n| n.to_str()).unwrap_or("sdkm").to_string()
}

/// 清理 work_dir 下残留的临时副本：
/// - .bak 为旧版本备份机制的遗留文件（机制已取消）
/// - .discard/.bad 为替换中断留下的副本
fn clean_leftovers(work: &Path, name: &str) {
    let prefix = format!("{}.", name);
    let Ok(entries) = fs::read_dir(work) else {
        return;
    };
    for entry in entries.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        let ours = fname.starts_with(&prefix)
            && ["discard", "bad", "bak"]
                .iter()
                .any(|ext| fname.ends_with(&format!(".{}", ext)));
        if ours {
            let _ = fs::remove_file(entry.path());
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

/// spawn exe --version，返回 stdout（带超时，避免旧版 GUI 打开窗口后永久阻塞）
fn spawn_version(exe: &Path) -> Result<String> {
    let mut child = Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to spawn `{}` --version", exe.display()))?;

    let deadline = Instant::now() + VERSION_PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    bail!("`{} --version` exited with {}", exe.display(), status);
                }
                break;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("`{} --version` timed out", exe.display());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Failed to wait for `{}`: {}", exe.display(), e);
            }
        }
    }

    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout
            .read_to_string(&mut out)
            .with_context(|| format!("Failed to read `{} --version` output", exe.display()))?;
    }
    Ok(out)
}

/// `--version` 输出中是否包含目标版本号（逐 token 比较，避免 0.5.1 命中 0.5.10）
fn reports_version(out: &str, target: &str) -> bool {
    out.split_whitespace().any(|tok| strip_v(tok) == target)
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
pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_ver(latest) > parse_ver(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_tokens_match_exactly() {
        assert!(reports_version("sdkm 0.5.6\n", "0.5.6"));
        assert!(reports_version("0.5.6\n", "0.5.6"));
        assert!(!reports_version("sdkm 0.5.60\n", "0.5.6"));
    }

    #[test]
    fn previous_ordering_prefers_highest_older() {
        assert!(is_newer("0.5.6", "0.5.5"));
        assert!(!is_newer("0.5.5", "0.5.6"));
    }
}
