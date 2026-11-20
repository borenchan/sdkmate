// 导入本地版本目录：把用户已有的 SDK 目录移交 sdkm store 统一托管
//
// 语义是「移动」而非「软链」：目录整体迁入 store/<sdk>/<version>，之后与远程安装的版本
// 完全同构（switch/uninstall/size 统计零特判）。store 是唯一真源——uninstall 直接
// remove_dir_all，若 store 内是外部软链会误删用户源目录或留下语义空洞。
// 不改动目录内部布局（用户选中的目录即版本根）；bin_dir 不可见时只发软警告（同 install Phase 9b）。

use crate::install::extractor::{locate_bin_dir, move_dir};
use crate::manager::SdkManager;
use anyhow::{Context, Result, bail};
use sdkm_util::path::get_installed_sdks_dir;
use sdkm_util::sdk::Sdk;
use std::fs;
use std::path::{Path, PathBuf};

impl SdkManager {
    /// 导入本地版本目录（把用户已有的 SDK 版本目录移动到 store 托管）
    ///
    /// - 版本名会直接成为 store 下的目录名：挡路径穿越与隐藏名
    /// - 版本名唯一：目标目录已存在直接拒绝
    /// - 链接/junction 源目录拒绝（store 必须是真实目录，move 链接会把激活环境搬走）
    /// - 跨盘时 move_dir 自动退化为 copy+delete（等效移动）；失败回滚半成品
    pub fn import_local_version(&self, sdk: &Sdk, src_dir: &Path, version: &str) -> Result<()> {
        let version = version.trim();
        if version.is_empty() {
            bail!("version name must not be empty");
        }
        if version.contains('/')
            || version.contains('\\')
            || version == "."
            || version == ".."
            || version.starts_with('.')
        {
            bail!(
                "invalid version name `{}`: must not contain path separators or start with '.'",
                version
            );
        }
        // 用 symlink_metadata：软链/junction 本身 is_dir() 为 false，先挡链接再判目录
        let meta = fs::symlink_metadata(src_dir)
            .with_context(|| format!("source directory `{}` is not accessible", src_dir.display()))?;
        if meta.file_type().is_symlink() {
            bail!(
                "source `{}` is a link/junction; choose the real version directory (sdkm-managed links are not importable)",
                src_dir.display()
            );
        }
        if !meta.is_dir() {
            bail!("source directory `{}` does not exist or is not a directory", src_dir.display());
        }

        let target_dir = get_installed_sdks_dir()?.join(sdk.to_string()).join(version);
        // 版本名唯一：同版本目录已存在则拒绝（不覆盖、不合并）
        if target_dir.exists() {
            bail!(
                "`{}` version `{}` already exists at `{}`; choose another version name or uninstall it first",
                sdk,
                version,
                target_dir.display()
            );
        }
        // 防自吞：目标不能落在源目录内部（否则 rename 失败后的递归拷贝会自我复制）。
        // canonicalize 在 Windows 返回 `\\?\` 前缀且路径大小写不敏感，需归一后比较
        let src_abs = fs::canonicalize(src_dir).unwrap_or_else(|_| src_dir.to_path_buf());
        if is_within(&target_dir, &src_abs) || is_within(&target_dir, src_dir) {
            bail!(
                "source directory `{}` contains the sdkm store; choose the version directory itself",
                src_dir.display()
            );
        }

        self.sink
            .info(format!("importing `{}` `{}` from `{}`...", sdk, version, src_dir.display()));
        if let Err(e) = move_dir(src_dir, &target_dir) {
            // 回滚半成品：跨盘 copy 中断会留部分文件；目标此前已校验不存在，可安全删除
            let _ = fs::remove_dir_all(&target_dir);
            return Err(e.context(format!("failed to move `{}` into store, nothing imported", src_dir.display())));
        }

        // bin_dir 布局软警告：与 install Phase 9b 同口径，只提示不阻断
        let bin_dir = self.config.find_sdk_ok(sdk)?.bin_dir.clone();
        if let Some(d) = bin_dir.as_deref()
            && !target_dir.join(d).is_dir()
        {
            match locate_bin_dir(&target_dir, d) {
                // 前缀 "bin dir mismatch for `<sdk>`" 与 install Phase 9b 同口径（GUI 解析锚点）
                Some(rel) => self.sink.warning(format!(
                    "bin dir mismatch for `{}`: '{}' not at version root (found at '{}') — fix: sdkm config set sdk.{}.bin_dir {}",
                    sdk,
                    d,
                    rel.display(),
                    sdk,
                    rel.display()
                )),
                None => self.sink.warning(format!(
                    "bin dir mismatch for `{}`: '{}' not found in '{}' — unexpected layout, SDK may be unusable",
                    sdk,
                    d,
                    target_dir.display()
                )),
            }
        }

        self.sink.success(format!("imported `{}` version `{}` to store", sdk, version));
        Ok(())
    }
}

/// child 是否位于 parent 之内（含相等）：先归一化路径形态再按组件比较
fn is_within(child: &Path, parent: &Path) -> bool {
    normalize_for_compare(child).starts_with(normalize_for_compare(parent))
}

/// 路径归一化（比较专用）：Windows 剥 verbatim 前缀 + 统一分隔符 + 小写（大小写不敏感）；
/// Unix 原样返回（大小写敏感，仅 canonicalize 结果即可）
fn normalize_for_compare(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
        return PathBuf::from(s.replace('/', "\\").to_lowercase());
    }
    #[cfg(not(windows))]
    p.to_path_buf()
}
