//! `sdkm current` 的三层生效版本解析（会话 > 项目 > 全局）。
//!
//! 优先级语义镜像 `shell/env.rs` 的 `generate_env_script_inner`——含「会话 pin 失效
//! （未装/匹配不上）时压制项目层直落全局」的规则，改解析优先级时两边必须同步。
//! 此处只做只读查询，不构造 env 数据，hook 热路径零接触。

use crate::manager::SdkManager;
use crate::project_config::{ProjectConfig, find_project_config};
use crate::shell::env::session_env_value;
use crate::version::fuzzy_match_version_core;
use anyhow::Result;
use std::fmt;
use std::path::PathBuf;
use util::path::get_installed_sdks_dir;
use util::sdk::Sdk;

/// 生效版本来源（三层作用域）
#[derive(Debug, Clone, PartialEq)]
pub enum VersionOrigin {
    /// 会话层 `SDKM_ACTIVE_<SDK>`（sdkm use --shell）
    Session,
    /// 项目层 `.sdkm.toml` pin（附配置文件路径）
    Project(PathBuf),
    /// 全局层 config.toml `current_version`
    Global,
}

impl fmt::Display for VersionOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionOrigin::Session => write!(f, "session"),
            VersionOrigin::Project(path) => write!(f, "project ({})", path.display()),
            VersionOrigin::Global => write!(f, "global"),
        }
    }
}

/// 一个 SDK 的实际生效版本：模糊解析后的精确版本 + 来源层
#[derive(Debug, Clone)]
pub struct EffectiveVersion {
    pub version: String,
    pub origin: VersionOrigin,
}

/// 全部已装 SDK 的生效版本清单（供 UI 层渲染，core 不做任何展示）
pub struct EffectiveVersions {
    /// 已注册 SDK 的 (sdk, 生效版本) 列表（store 目录序）
    pub entries: Vec<(Sdk, Option<EffectiveVersion>)>,
    /// 未注册的 store 目录 (目录名, 校验错误)，UI 层自行提示
    pub skipped: Vec<(String, String)>,
}

impl SdkManager {
    /// 解析单个 SDK 的三层生效版本（供 `sdkm current` 单 SDK 查询）
    pub fn effective_version(&self, sdk: &Sdk) -> Option<EffectiveVersion> {
        // Err（cwd 不可得）当无项目层，与 env.rs 的 find_project_config().ok().flatten() 一致
        let project = find_project_config().ok().flatten();
        self.effective_version_with(sdk, project.as_ref())
    }

    /// 同上，复用调用方已取的项目配置（全 SDK 表格只向上找一次目录链）
    pub(crate) fn effective_version_with(
        &self,
        sdk: &Sdk,
        project: Option<&(PathBuf, ProjectConfig)>,
    ) -> Option<EffectiveVersion> {
        let name = sdk.to_string();
        // 会话层最高优先；pin 失效（未装/匹配不上）时压制项目层直落全局（镜像 env.rs 优先级）
        if let Some(input) = session_env_value(&name, None) {
            return match self.resolve_installed(sdk, &input) {
                Some(version) => Some(EffectiveVersion {
                    version,
                    origin: VersionOrigin::Session,
                }),
                None => self.global_version(sdk),
            };
        }
        // 项目层：.sdkm.toml pin；失效静默降级全局
        if let Some((path, cfg)) = project
            && let Some(input) = cfg.pins.get(&name)
        {
            return match self.resolve_installed(sdk, input) {
                Some(version) => Some(EffectiveVersion {
                    version,
                    origin: VersionOrigin::Project(path.clone()),
                }),
                None => self.global_version(sdk),
            };
        }
        // 全局层兜底
        self.global_version(sdk)
    }

    /// 模糊解析本地已装版本（与 switch/use 同源匹配）；未装/无匹配 → None
    fn resolve_installed(&self, sdk: &Sdk, input: &str) -> Option<String> {
        let versions = self.list_local_sdk_versions(sdk).ok()?;
        if versions.is_empty() {
            return None;
        }
        let version_strings: Vec<String> = versions.iter().map(|v| v.sdk_version.clone()).collect();
        fuzzy_match_version_core(&version_strings, input).ok().map(|m| m.full_version)
    }

    /// 全局层：`current_version` 原样返回（不重校验安装，与现状一致）
    fn global_version(&self, sdk: &Sdk) -> Option<EffectiveVersion> {
        self.config
            .find_sdk(sdk)?
            .current_version
            .clone()
            .map(|version| EffectiveVersion {
                version,
                origin: VersionOrigin::Global,
            })
    }

    /// 收集全部已装 SDK 的生效版本（遍历 store 目录；项目配置只向上找一次）
    pub fn effective_versions_all(&self) -> Result<EffectiveVersions> {
        let project = find_project_config().ok().flatten();
        let mut entries = Vec::new();
        let mut skipped = Vec::new();
        for entry in get_installed_sdks_dir()?.read_dir()?.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            // store 目录可能含未在 config 注册的子目录(如手动放入或遗留),跳过并记录
            let sdk = match self.match_valid_sdk(&name) {
                Ok(s) => s,
                Err(e) => {
                    skipped.push((name, e.to_string()));
                    continue;
                }
            };
            entries.push((sdk.clone(), self.effective_version_with(&sdk, project.as_ref())));
        }
        Ok(EffectiveVersions { entries, skipped })
    }
}
