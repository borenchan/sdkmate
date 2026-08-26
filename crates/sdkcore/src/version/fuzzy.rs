// 版本字符串模糊匹配(与 SDK 来源无关,install/switch/list 共用)

use anyhow::Result;
use std::cmp::Ordering;

/// 去除版本号前导的 `v`/`V`(`"v14.16.0"` → `"14.16.0"`),用于归一化比较。
///
/// 仅当 `v` 后紧跟数字时才剥离,避免误伤 `"version"` 之类非版本串。
/// 返回值是入参的切片,不分配。
fn strip_v_prefix(s: &str) -> &str {
    let trimmed = s.trim();
    match trimmed.strip_prefix(['v', 'V']) {
        Some(rest) if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) => rest,
        _ => trimmed,
    }
}

/// 通用模糊匹配结果(与 SDK 来源无关,install/switch 共用)
pub struct FuzzyMatch {
    /// 匹配到的完整版本号(保留原始形态,如带 `v` 前缀)
    pub full_version: String,
    /// 是否为模糊匹配的结果(需要交互确认)
    pub fuzzy_matched: bool,
}

/// 通用版本模糊匹配核心:精确 → 前缀模糊 → 失败(带相近版本建议)
///
/// - 精确匹配:`"3.12.10"` → 直接命中
/// - 模糊 `"3"` → 最新 `3.x.x`
/// - 模糊 `"3.12"` → 最新 `3.12.x`
/// - `v` 前缀归一化:`"14"` 可命中 `"v14.16.0"`(Node 等 release tag 带 `v`)
///
/// `versions` 任意顺序,内部按版本号降序排列后取最新。
/// 返回 `fuzzy_matched=true` 表示模糊匹配结果,需要交互确认。
pub fn fuzzy_match_version_core(versions: &[String], version_input: &str) -> Result<FuzzyMatch> {
    if versions.is_empty() {
        return Err(no_match_error(versions, version_input));
    }

    // 降序排列(取第一个 = 最新)
    let mut sorted: Vec<&String> = versions.iter().collect();
    sorted.sort_by(|a, b| compare_versions_desc(a, b));

    // 归一化输入(剥离 v 前缀),后续精确/前缀匹配均基于归一化形式
    let norm_input = strip_v_prefix(version_input);

    // 1. 精确匹配优先(归一化后比较,故 "14.16.0" == "v14.16.0")
    if let Some(v) = sorted.iter().find(|v| strip_v_prefix(v.as_str()) == norm_input) {
        return Ok(FuzzyMatch {
            full_version: v.to_string(),
            fuzzy_matched: false,
        });
    }

    // 2. 前缀模糊:input 作为更长版本号的前缀
    //    `"3" → "3."`、`"3.12" → "3.12."`
    //    `"3.1."` 不会误匹配 `"3.10.x"`(第 4 字符是 '0' 而非 '.')
    //    归一化后比较,故 "14" 可前缀匹配 "v14.16.0" → "14.16.0"
    let prefix = format!("{}.", norm_input);
    if let Some(v) = sorted.iter().find(|v| strip_v_prefix(v.as_str()).starts_with(&prefix)) {
        return Ok(FuzzyMatch {
            full_version: v.to_string(),
            fuzzy_matched: true,
        });
    }

    // 3. 无匹配 → 带相近版本建议的错误
    Err(no_match_error(versions, version_input))
}

/// 在可用版本中找与 input 最相近者(匹配失败时提示用)
///
/// 主排序:与 input 的最长公共数值前缀(越长越好);
/// 次排序:数值距离(越小越好,高位分量权重更高)。
pub fn suggest_similar_version(versions: &[String], input: &str) -> Option<String> {
    if versions.is_empty() {
        return None;
    }
    let input_parts = parse_version_components(input);

    let mut best: Option<(usize, u64, &String)> = None;
    for v in versions {
        let parts = parse_version_components(v);
        // 最长公共前缀(数值分量逐段比较)
        let common = input_parts.iter().zip(parts.iter()).take_while(|(a, b)| a == b).count();
        let dist = version_distance(&input_parts, &parts);
        let candidate = (common, dist, v);

        best = Some(match best {
            None => candidate,
            Some(b) => {
                // 公共前缀越长越好;相同时距离越小越好
                let better = (candidate.0 > b.0) || (candidate.0 == b.0 && candidate.1 < b.1);
                if better { candidate } else { b }
            }
        });
    }
    best.map(|(_, _, v)| v.clone())
}

/// 构造"未匹配"错误:有相近版本则附加 "did you mean" 提示
fn no_match_error(versions: &[String], input: &str) -> anyhow::Error {
    match suggest_similar_version(versions, input) {
        Some(s) => anyhow::anyhow!("version '{}' not found in available versions, did you mean '{}'?", input, s),
        None => anyhow::anyhow!("version '{}' not found in available versions", input),
    }
}

/// 解析版本号为数值分量(`"3.12.10"` → `[3, 12, 10]`),非数值段跳过。
/// 先剥离 `v` 前缀,故 `"v14.16.0"` → `[14, 16, 0]`(不会丢失主版本号)。
fn parse_version_components(v: &str) -> Vec<u32> {
    strip_v_prefix(v).split('.').filter_map(|s| s.parse().ok()).collect()
}

/// 按版本号降序比较(数值分量逐段比较)
fn compare_versions_desc(a: &str, b: &str) -> Ordering {
    let va = parse_version_components(a);
    let vb = parse_version_components(b);
    vb.cmp(&va) // 降序:b 在前则 b 更大
}

/// 计算两版本号的数值距离(高位分量权重更高,使第一个不同分量主导结果)
fn version_distance(a: &[u32], b: &[u32]) -> u64 {
    let n = a.len().max(b.len());
    let mut d: u64 = 0;
    for i in 0..n {
        let av = a.get(i).copied().unwrap_or(0) as u64;
        let bv = b.get(i).copied().unwrap_or(0) as u64;
        // 高位权重高:saturating_pow 防止分量过多时溢出
        let weight = 100u64.saturating_pow((n - i - 1) as u32);
        d += av.abs_diff(bv) * weight;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn strip_v_prefix_only_strips_when_followed_by_digit() {
        assert_eq!(strip_v_prefix("v14.16.0"), "14.16.0");
        assert_eq!(strip_v_prefix("V14.16.0"), "14.16.0");
        assert_eq!(strip_v_prefix("14.16.0"), "14.16.0");
        // v 后非数字 → 不剥离
        assert_eq!(strip_v_prefix("version"), "version");
    }

    #[test]
    fn exact_match_no_fuzzy_flag() {
        let versions = vs(&["3.11.6", "3.12.10", "3.12.0"]);
        let m = fuzzy_match_version_core(&versions, "3.12.10").unwrap();
        assert_eq!(m.full_version, "3.12.10");
        assert!(!m.fuzzy_matched);
    }

    #[test]
    fn v_prefixed_exact_match_normalizes() {
        // 输入无 v、候选带 v → 归一化后精确命中,不算模糊
        let versions = vs(&["v14.16.0", "v16.20.0"]);
        let m = fuzzy_match_version_core(&versions, "14.16.0").unwrap();
        assert_eq!(m.full_version, "v14.16.0");
        assert!(!m.fuzzy_matched);
    }

    #[test]
    fn v_prefixed_input_matches_v_prefixed_candidate() {
        // 输入带 v、候选带 v → 精确命中
        let versions = vs(&["v14.16.0"]);
        let m = fuzzy_match_version_core(&versions, "v14.16.0").unwrap();
        assert_eq!(m.full_version, "v14.16.0");
        assert!(!m.fuzzy_matched);
    }

    #[test]
    fn prefix_fuzzy_picks_latest() {
        let versions = vs(&["3.12.0", "3.12.10", "3.11.6"]);
        let m = fuzzy_match_version_core(&versions, "3.12").unwrap();
        assert_eq!(m.full_version, "3.12.10");
        assert!(m.fuzzy_matched);
    }

    #[test]
    fn prefix_fuzzy_major_only() {
        let versions = vs(&["3.11.6", "3.12.10", "4.0.0"]);
        let m = fuzzy_match_version_core(&versions, "3").unwrap();
        assert_eq!(m.full_version, "3.12.10");
        assert!(m.fuzzy_matched);
    }

    #[test]
    fn v_prefixed_prefix_fuzzy_matches() {
        // 报告的回归用例:sdkm s node 14,本地为 v14.16.0
        let versions = vs(&["v14.16.0", "v16.20.0", "v18.12.0"]);
        let m = fuzzy_match_version_core(&versions, "14").unwrap();
        assert_eq!(m.full_version, "v14.16.0");
        assert!(m.fuzzy_matched);
    }

    #[test]
    fn prefix_does_not_cross_version_boundary() {
        // "3.1" 不应匹配 "3.10.x"(前缀方案 "3.1." vs "3.10.")
        let versions = vs(&["3.10.0", "3.11.6"]);
        assert!(fuzzy_match_version_core(&versions, "3.1").is_err());
    }

    #[test]
    fn no_match_returns_did_you_mean_error() {
        // 输入 13 离 v14.16.0 最近(14 vs 16),应建议它
        let versions = vs(&["v14.16.0", "v16.20.0"]);
        let err = fuzzy_match_version_core(&versions, "13")
            .err()
            .expect("expected an error for unmatched version");
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
        assert!(msg.contains("did you mean"));
        assert!(msg.contains("v14.16.0"));
    }

    #[test]
    fn empty_versions_errors_without_suggestion() {
        let versions: Vec<String> = Vec::new();
        let err = fuzzy_match_version_core(&versions, "14")
            .err()
            .expect("expected an error when no versions available");
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
        assert!(!msg.contains("did you mean"));
    }

    #[test]
    fn parse_components_strips_v_prefix() {
        assert_eq!(parse_version_components("v14.16.0"), vec![14, 16, 0]);
        assert_eq!(parse_version_components("14.16.0"), vec![14, 16, 0]);
        assert_eq!(parse_version_components("3.12"), vec![3, 12]);
    }

    #[test]
    fn compare_desc_orders_newest_first_with_v_prefix() {
        // v14.16.0 < v16.20.0,降序排后 v16 在前
        let mut items = vec!["v14.16.0".to_string(), "v16.20.0".to_string()];
        items.sort_by(|a, b| compare_versions_desc(a, b));
        assert_eq!(items, vec!["v16.20.0".to_string(), "v14.16.0".to_string()]);
    }

    #[test]
    fn suggest_picks_closest_by_numeric_prefix() {
        let versions = vs(&["v14.16.0", "v16.20.0"]);
        // 归一化后 14 与 v14.16.0 有公共数值前缀 → 建议它
        assert_eq!(suggest_similar_version(&versions, "14"), Some("v14.16.0".into()));
    }
}
