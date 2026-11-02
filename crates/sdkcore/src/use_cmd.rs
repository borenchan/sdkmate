//! `sdkm use` 命令业务：设置项目级（默认）或会话级版本。
//!
//! - 项目级（默认）：写当前目录 `.sdkm.toml`（父级冲突 warning）。**要求本地已装且
//!   模糊匹配成功才写入**（写解析后的精确版本，与 switch 同源匹配逻辑）；未装/匹配
//!   失败拒绝写入——坏 pin 会让每次提示符渲染都解析失败降级全局。
//! - 会话级（`--shell`）：不写文件，输出 `SDKM_ACTIVE_<SDK>` 赋值行 + 即时 PATH 重建
//!   （LIFO 顶前）供 `eval "$(sdkm use --shell ...)"`。**此路径禁 stdout 宏**（stdout 只吐脚本）。

use crate::manager::SdkManager;
use crate::project_config::{ProjectConfig, read_project_config, write_project_config};
use crate::version::fuzzy_match_version_core;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::env;
use util::consts::PROJECT_CONFIG_FILE_NAME;
use util::sdk::Sdk;
use util::shell::Shell;
use util::{info, success};

impl SdkManager {
    /// 设置项目级版本：写当前目录 `.sdkm.toml`
    ///
    /// 本地已装且模糊匹配成功才写（写精确版本）；未装/匹配失败直接 Err 拒绝写入。
    pub fn use_project_version(&self, sdk: &Sdk, version_input: &str) -> Result<()> {
        // 校验 SDK 已注册（与 switch/install 同语义）
        self.config.find_sdk_ok(sdk)?;

        let cwd = env::current_dir().context("cannot get current directory")?;

        // 模糊匹配本地已装版本（与 switch 同源；Err 含未装提示/did-you-mean 建议直接冒泡）
        let resolved = self.resolve_local_version(sdk, version_input)?;

        // 读现有配置（当前目录已有 .sdkm.toml 则合并），写入
        let existing_path = cwd.join(PROJECT_CONFIG_FILE_NAME);
        let mut pins = if existing_path.is_file() {
            read_project_config(&existing_path).pins
        } else {
            BTreeMap::new()
        };
        pins.insert(sdk.to_string(), resolved.clone());

        write_project_config(&cwd, &ProjectConfig { pins })?;

        info!("Pinned `{}` = `{}` in {}", sdk, resolved, existing_path.display());
        success!("Project config updated. Open a new shell (or cd out and back in) to activate.");
        Ok(())
    }

    /// 设置会话级版本：返回 eval 脚本（SDKM_ACTIVE 赋值 + 即时 PATH 重建）
    ///
    /// 显式操作按 LIFO 顶到 PATH 最前（用户最新意图优先）；之后 hook 以新位置为锚。
    /// **禁 stdout 宏**：返回值由 CLI 层 println 到 stdout 供 eval，诊断走 stderr。
    pub fn use_session_version(&self, shell: Shell, sdk: &Sdk, version_input: &str) -> Result<String> {
        let resolved = self.resolve_local_version(sdk, version_input)?;
        Ok(self.generate_use_script(shell, &sdk.to_string(), &resolved))
    }

    /// 模糊匹配本地已装版本（与 switch 完全同源，不新增匹配逻辑）
    ///
    /// Ok = 解析后的精确版本；本地无版本/fuzzy 失败 → Err（含安装提示/did-you-mean 建议）
    fn resolve_local_version(&self, sdk: &Sdk, version_input: &str) -> Result<String> {
        let versions = self.list_local_sdk_versions(sdk)?;
        if versions.is_empty() {
            bail!(
                "`{}` has no installed versions. Install it first (`sdkm install {} {}`)",
                sdk,
                sdk,
                version_input
            );
        }
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        let matched = fuzzy_match_version_core(&version_strings, version_input)?;
        Ok(matched.full_version)
    }
}
