//! `sdkm use` 命令业务：设置项目级（默认）或会话级版本。
//!
//! - 项目级（默认）：写当前目录 `.sdkm.toml`（父级冲突 warning）。写入即声明意图，
//!   不强制安装——未装时 `sdkm env` 自动降级回全局。
//! - 会话级（`--shell`）：不写文件，输出 `export SDKM_ACTIVE_<SDK>=<version>` 供
//!   `eval "$(sdkm use --shell ...)"`。**此路径禁 stdout 宏**（stdout 只吐脚本）。

use crate::manager::SdkManager;
use crate::project_config::{ProjectConfig, read_project_config, write_project_config};
use crate::version::fuzzy_match_version_core;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::env;
use util::consts::{PROJECT_CONFIG_FILE_NAME, SDKM_SESSION_ENV_PREFIX};
use util::sdk::Sdk;
use util::shell::Shell;
use util::{error, info, success, warning};

impl SdkManager {
    /// 设置项目级版本：写当前目录 `.sdkm.toml`
    pub fn use_project_version(&self, sdk: &Sdk, version_input: &str) -> Result<()> {
        // 校验 SDK 已注册（与 switch/install 同语义）
        self.config.find_sdk_ok(sdk)?;

        let cwd = env::current_dir().context("cannot get current directory")?;

        // 模糊匹配本地已装版本（有装才解析精确版本；未装仍写入——声明意图不强制安装）
        let installed_hint = match self.resolve_local_version(sdk, version_input) {
            Ok(Some(resolved)) => resolved,
            Ok(None) => {
                warning!(
                    "`{}` version `{}` is not installed locally. It will be written as intent; \
                     `sdkm env` will fall back to global until you install it (`sdkm install {} {}).",
                    sdk,
                    version_input,
                    sdk,
                    version_input
                );
                version_input.to_string()
            }
            Err(e) => return Err(e),
        };

        // 读现有配置（当前目录已有 .sdkm.toml 则合并），写入
        let existing_path = cwd.join(PROJECT_CONFIG_FILE_NAME);
        let mut pins = if existing_path.is_file() {
            read_project_config(&existing_path).pins
        } else {
            BTreeMap::new()
        };
        pins.insert(sdk.to_string(), installed_hint.clone());

        write_project_config(&cwd, &ProjectConfig { pins })?;

        info!("Pinned `{}` = `{}` in {}", sdk, installed_hint, existing_path.display());
        success!("Project config updated. Open a new shell (or cd out and back in) to activate.");
        Ok(())
    }

    /// 设置会话级版本：返回 eval 脚本（设置 `SDKM_ACTIVE_<SDK>` 环境变量）
    ///
    /// **禁 stdout 宏**：返回值由 CLI 层 println 到 stdout 供 eval，诊断走 stderr。
    pub fn use_session_version(&self, shell: Shell, sdk: &Sdk, version_input: &str) -> Result<String> {
        let Some(resolved) = self.resolve_local_version(sdk, version_input)? else {
            error!(
                "`{}` version `{}` is not installed locally. Install it first (`sdkm install {} {}), \
                 or the session override cannot resolve.",
                sdk, version_input, sdk, version_input
            );
            bail!("session version requires a locally installed version");
        };

        let var = format!("{}{}", SDKM_SESSION_ENV_PREFIX, sanitize_env_suffix(&sdk.to_string()));
        // 按 shell 语法后端输出赋值行（bash `export` / fish `set -gx` / PS `$env:`）
        let script = format!("{}\n", (shell.syntax().export_line)(&var, &resolved));
        Ok(script)
    }

    /// 模糊匹配本地已装版本：已装返 Some(精确版本)；未装返 None；其他错误 Err
    fn resolve_local_version(&self, sdk: &Sdk, version_input: &str) -> Result<Option<String>> {
        let versions = self.list_local_sdk_versions(sdk)?;
        if versions.is_empty() {
            return Ok(None);
        }
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        match fuzzy_match_version_core(&version_strings, version_input) {
            Ok(m) => Ok(Some(m.full_version)),
            Err(e) => {
                // fuzzy 失败 = 本地没有任何相近版本 → 当作未安装（写入意图 / 会话报错）
                let _ = e;
                Ok(None)
            }
        }
    }
}

/// SDK 名转合法 env var 后缀：大写 + 非字母数字替 `_`（custom SDK 可含 `-`）
fn sanitize_env_suffix(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}
