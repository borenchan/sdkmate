//! 项目级配置 `.sdkm.toml`：摊平 KV（`java = "21"`），会话 > 项目 > 全局三层中的项目层。
//!
//! 读（runtime）：从 PWD 向上递归找第一个 `.sdkm.toml` 命中即停；格式错误静默降级
//! （返空配置，不阻断 shell hook）。写（user action）：只写当前目录；写前向上探测
//! 父级配置，命中则 warning 提示覆盖关系。原子写（tmp+rename）。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use util::consts::PROJECT_CONFIG_FILE_NAME;
use util::warning;

/// 项目级配置：SDK 名 → 期望版本（摊平 KV，BTreeMap 序列化按 key 排序保证确定性）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(flatten)]
    pub pins: BTreeMap<String, String>,
}

/// 从当前目录向上递归查找最近的 `.sdkm.toml`，命中即停
///
/// 记录已访问路径防符号链接环。返回 (配置文件路径, 解析结果)；到根未找到返 None。
/// 文件存在但格式错误 → 返回 (路径, 空配置)（静默降级，不阻断 hook）。
pub fn find_project_config() -> Result<Option<(PathBuf, ProjectConfig)>> {
    let mut dir = env::current_dir().context("cannot get current directory")?;
    let mut visited: Vec<PathBuf> = Vec::new();

    loop {
        // 已访问过（符号链接环）→ 停止查找
        if visited.contains(&dir) {
            return Ok(None);
        }
        visited.push(dir.clone());

        let config_path = dir.join(PROJECT_CONFIG_FILE_NAME);
        if config_path.is_file() {
            let cfg = read_project_config(&config_path);
            return Ok(Some((config_path, cfg)));
        }

        // 到根目录仍未找到
        if !dir.pop() {
            return Ok(None);
        }
    }
}

/// 解析项目配置：格式错误/读失败 → 空配置（静默降级，不抛错阻断 hook）
pub fn read_project_config(path: &Path) -> ProjectConfig {
    let Ok(content) = fs::read_to_string(path) else {
        return ProjectConfig::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

/// 写项目配置到指定目录（原子写 tmp+rename）
///
/// 写前向上探测父级是否已有 `.sdkm.toml`，命中则 warning 提示覆盖关系。
pub fn write_project_config(dir: &Path, cfg: &ProjectConfig) -> Result<()> {
    let config_path = dir.join(PROJECT_CONFIG_FILE_NAME);

    // 父级冲突检测：当前目录自身配置不算，只看更上层
    warn_parent_config(dir)?;

    let content = toml::to_string_pretty(cfg).context("Failed to serialize project config to TOML")?;

    // 原子写：临时文件 + rename（照搬 config.toml 的写入策略）
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let tmp_name = format!("{}.tmp.{:016x}", PROJECT_CONFIG_FILE_NAME, ts & 0xFFFFFFFFFFFFFFFF);
    let tmp_path = dir.join(&tmp_name);
    fs::write(&tmp_path, &content).context("Failed to write temporary project config")?;
    if let Err(e) = fs::rename(&tmp_path, &config_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e).context("Failed to rename temporary project config to final path");
    }
    Ok(())
}

/// 向上探测父级目录是否已存在 `.sdkm.toml`，存在则 warning（只提示不阻断）
fn warn_parent_config(dir: &Path) -> Result<()> {
    let mut parent = dir.to_path_buf();
    if !parent.pop() {
        return Ok(());
    }
    let mut visited: Vec<PathBuf> = Vec::new();
    loop {
        if visited.contains(&parent) {
            return Ok(());
        }
        visited.push(parent.clone());

        let config_path = parent.join(PROJECT_CONFIG_FILE_NAME);
        if config_path.is_file() {
            warning!(
                "Found parent config at '{}'. Creating a new config here will override it for this directory.",
                config_path.display()
            );
            return Ok(());
        }
        if !parent.pop() {
            return Ok(());
        }
    }
}

/// 便捷方法：HashMap 转换（供上层以无序方式操作 pins）
pub fn pins_from_map(map: HashMap<String, String>) -> ProjectConfig {
    ProjectConfig {
        pins: map.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flat_kv() {
        let cfg: ProjectConfig = toml::from_str("java = \"21\"\nnode = \"20.11.0\"\n").unwrap();
        assert_eq!(cfg.pins.get("java").map(String::as_str), Some("21"));
        assert_eq!(cfg.pins.get("node").map(String::as_str), Some("20.11.0"));
    }

    #[test]
    fn unknown_key_tolerated() {
        // 未知 key（非 SDK）被忽略，不报错
        let cfg: ProjectConfig = toml::from_str("java = \"21\"\nnotaconfig = \"x\"\n").unwrap();
        assert_eq!(cfg.pins.len(), 2);
    }

    #[test]
    fn malformed_toml_returns_default() {
        let cfg: ProjectConfig = toml::from_str("java = [broken").unwrap_or_default();
        assert!(cfg.pins.is_empty());
    }

    #[test]
    fn serialize_sorted() {
        // BTreeMap 保证序列化按 key 字母序，输出确定
        let cfg = pins_from_map(HashMap::from([("node".into(), "20".into()), ("java".into(), "21".into())]));
        let s = toml::to_string_pretty(&cfg).unwrap();
        let java_pos = s.find("java").unwrap();
        let node_pos = s.find("node").unwrap();
        assert!(java_pos < node_pos);
    }
}
