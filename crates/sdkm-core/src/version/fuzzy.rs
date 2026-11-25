// 版本字符串模糊匹配(与 SDK 来源无关,install/switch/list 共用)

use anyhow::{Context, Result};
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
#[derive(Debug)]
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
/// - 家族优先防劫持:输入恰好能精确命中裸短版本条目但列表另有 `"N."` 家族时
///   (如 go.dev 的 `go1` → `"1"`,列表另有 `1.27.3`),取家族最新而非裸条目
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

    let prefix = format!("{}.", norm_input);
    // 是否存在以输入为前缀的更长版本家族(如 "1" → "1.27.3")
    let has_family = sorted.iter().any(|v| strip_v_prefix(v.as_str()).starts_with(&prefix));

    // 1. 精确匹配优先(归一化后比较,故 "14.16.0" == "v14.16.0")
    //    家族存在时跳过——裸短版本条目(如 go.dev 的 go1 → "1")几乎总是意图取该系列最新
    if !has_family && let Some(v) = sorted.iter().find(|v| strip_v_prefix(v.as_str()) == norm_input) {
        return Ok(FuzzyMatch {
            full_version: v.to_string(),
            fuzzy_matched: false,
        });
    }

    // 2. 前缀模糊:input 作为更长版本号的前缀
    //    `"3" → "3."`、`"3.12" → "3.12."`
    //    `"3.1."` 不会误匹配 `"3.10.x"`(第 4 字符是 '0' 而非 '.')
    //    归一化后比较,故 "14" 可前缀匹配 "v14.16.0" → "14.16.0"
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
pub(super) fn compare_versions_desc(a: &str, b: &str) -> Ordering {
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

// ─── Java 多供应商（sdkman 式版本标识）──────────────────────────

/// JDK 供应商（版本后缀的取值域；sdkman 式标识见 parse_java_version_input）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaVendor {
    /// Eclipse Temurin（默认，存储无后缀，兼容既有安装）
    Temurin,
    /// Azul Zulu
    Zulu,
    /// Amazon Corretto
    Corretto,
    /// BellSoft Liberica
    Liberica,
}

impl JavaVendor {
    pub const ALL: [JavaVendor; 4] = [
        JavaVendor::Temurin,
        JavaVendor::Zulu,
        JavaVendor::Corretto,
        JavaVendor::Liberica,
    ];

    /// 存储规范后缀（Temurin 无后缀）
    pub fn suffix(self) -> Option<&'static str> {
        match self {
            JavaVendor::Temurin => None,
            JavaVendor::Zulu => Some("zulu"),
            JavaVendor::Corretto => Some("amzn"),
            JavaVendor::Liberica => Some("librca"),
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            JavaVendor::Temurin => "Temurin",
            JavaVendor::Zulu => "Zulu",
            JavaVendor::Corretto => "Corretto",
            JavaVendor::Liberica => "Liberica",
        }
    }

    /// 别名 → 供应商（大小写不敏感）
    pub fn from_alias(s: &str) -> Option<JavaVendor> {
        match s.to_ascii_lowercase().as_str() {
            "tem" | "temurin" | "eclipse" => Some(JavaVendor::Temurin),
            "zulu" => Some(JavaVendor::Zulu),
            "amzn" | "corretto" | "amazon" => Some(JavaVendor::Corretto),
            "librca" | "liberica" | "bellsoft" => Some(JavaVendor::Liberica),
            _ => None,
        }
    }
}

/// 从版本串尾部拆已知供应商后缀；未知尾段（如 `1.0.0-beta`）不拆，返回 None
fn split_vendor_suffix(version: &str) -> Option<(&str, JavaVendor)> {
    version
        .rsplit_once('-')
        .and_then(|(num, tail)| JavaVendor::from_alias(tail).map(|v| (num, v)))
}

/// 解析用户输入 → (数字版本, 供应商)
///
/// 支持三种写法：`21`/`21.0.13`（默认 Temurin）、`21-zulu`（后缀式）、`zulu@17`（供应商@版本，
/// `@` 优先于后缀）。`@` 左侧必须是已知别名，右侧不再允许供应商后缀（防冲突写法）。
pub fn parse_java_version_input(input: &str) -> Result<(String, JavaVendor)> {
    let t = input.trim();
    if let Some((lhs, rhs)) = t.split_once('@') {
        let vendor = JavaVendor::from_alias(lhs.trim()).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown Java vendor '{}'. Valid: tem/temurin, zulu, amzn/corretto, librca/liberica",
                lhs.trim()
            )
        })?;
        let ver = rhs.trim();
        if ver.is_empty() {
            anyhow::bail!("missing version after '@' (e.g. `zulu@17`)");
        }
        if let Some((num, v2)) = split_vendor_suffix(ver) {
            if v2 != vendor {
                anyhow::bail!(
                    "conflicting vendor spec in '{}': '@{}' vs '-{}'",
                    input,
                    vendor.display_name().to_lowercase(),
                    v2.display_name().to_lowercase()
                );
            }
            return Ok((num.to_string(), vendor));
        }
        return Ok((ver.to_string(), vendor));
    }
    Ok(match split_vendor_suffix(t) {
        Some((num, v)) => (num.to_string(), v),
        None => (t.to_string(), JavaVendor::Temurin),
    })
}

/// 规范存储形态：数字版本 + 供应商后缀（Temurin 无后缀）
pub fn format_java_version(numeric: &str, vendor: JavaVendor) -> String {
    match vendor.suffix() {
        Some(s) => format!("{numeric}-{s}"),
        None => numeric.to_string(),
    }
}

/// 供应商感知的本地版本模糊匹配（store 目录名带后缀，如 `21.0.13-zulu`）
///
/// - 输入先过 [`parse_java_version_input`]（`@` 与后缀两种写法都支持）
/// - 候选按供应商过滤（无已知后缀 = Temurin），对数字部分跑 [`fuzzy_match_version_core`]
/// - 裸 major（`21` / `zulu@21`）同样走前缀匹配（fuzzy=true，调用端确认，与历史行为一致）
/// - 目标供应商无候选时，错误里列出已装的其他供应商版本并给出切换提示
pub fn fuzzy_match_version_with_vendor(versions: &[String], input: &str) -> Result<FuzzyMatch> {
    let (numeric_in, vendor) = parse_java_version_input(input)?;

    // 候选拆（原始串, 数字部分, 供应商）
    let pool: Vec<(&String, &str, JavaVendor)> = versions
        .iter()
        .map(|v| match split_vendor_suffix(v) {
            Some((num, ven)) => (v, num, ven),
            None => (v, v.as_str(), JavaVendor::Temurin),
        })
        .collect();
    let filtered: Vec<(&String, &str, JavaVendor)> = pool.iter().copied().filter(|(_, _, v)| *v == vendor).collect();

    if filtered.is_empty() {
        let others: Vec<&str> = pool
            .iter()
            .copied()
            .filter(|(_, _, v)| *v != vendor)
            .map(|(orig, _, _)| orig.as_str())
            .collect();
        let list = if others.is_empty() {
            String::new()
        } else {
            format!(" (installed: {})", others.join(", "))
        };
        let suggest = others.first().map(|o| format!(", try '{o}'")).unwrap_or_default();
        anyhow::bail!(
            "version '{input}' not found: no installed versions for vendor '{}'{}{}",
            vendor.display_name(),
            list,
            suggest
        );
    }

    let nums: Vec<String> = filtered.iter().map(|(_, n, _)| (*n).to_string()).collect();
    // 裸 major（"21"）与部分版本（"21.0"）都走核心前缀匹配（fuzzy=true 需确认，与历史行为一致）
    let matched = match fuzzy_match_version_core(&nums, &numeric_in) {
        Ok(m) => m,
        Err(e) => {
            // 同供应商无匹配时，若其他供应商装了同系列版本，给出可操作的切换提示
            let others: Vec<&str> = pool
                .iter()
                .copied()
                .filter(|(_, _, v)| *v != vendor)
                .map(|(orig, _, _)| orig.as_str())
                .collect();
            if others.is_empty() {
                return Err(e);
            }
            let suggest = others
                .iter()
                .find(|o| {
                    let num = split_vendor_suffix(o).map(|(n, _)| n).unwrap_or(o);
                    num.starts_with(&format!("{numeric_in}."))
                })
                .or_else(|| others.first())
                .copied()
                .unwrap_or("");
            anyhow::bail!("{}; other vendors installed: {} (try '{}')", e, others.join(", "), suggest)
        }
    };

    let original = filtered
        .iter()
        .find(|(_, n, _)| *n == matched.full_version)
        .map(|(orig, _, _)| (*orig).clone())
        .context("matched version not found in candidate pool (internal error)")?;
    Ok(FuzzyMatch {
        full_version: original,
        fuzzy_matched: matched.fuzzy_matched,
    })
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
    fn bare_short_version_not_hijacked_when_family_exists() {
        // 回归:sdkm i go 1——go.dev 列表混入裸 "1"(go1.0,仅 bootstrap 源码),
        // 输入 "1" 应取 1.x 家族最新,而非精确命中裸条目拼出 go1.windows-amd64.zip → 404
        let versions = vs(&["1", "1.26.8", "1.27.3"]);
        let m = fuzzy_match_version_core(&versions, "1").unwrap();
        assert_eq!(m.full_version, "1.27.3");
        assert!(m.fuzzy_matched);
    }

    #[test]
    fn bare_version_exact_when_no_family() {
        // 列表只有裸短版本、无家族 → 正常精确命中(Java major 风格数据)
        let versions = vs(&["17", "21"]);
        let m = fuzzy_match_version_core(&versions, "21").unwrap();
        assert_eq!(m.full_version, "21");
        assert!(!m.fuzzy_matched);
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

    // ── Java 供应商后缀匹配 ──────────────────────────────────────

    #[test]
    fn parse_java_input_forms() {
        assert_eq!(parse_java_version_input("21").unwrap(), ("21".into(), JavaVendor::Temurin));
        assert_eq!(
            parse_java_version_input("21.0.13").unwrap(),
            ("21.0.13".into(), JavaVendor::Temurin)
        );
        assert_eq!(parse_java_version_input("21-zulu").unwrap(), ("21".into(), JavaVendor::Zulu));
        assert_eq!(
            parse_java_version_input("21.0.13.0.11-amzn").unwrap(),
            ("21.0.13.0.11".into(), JavaVendor::Corretto)
        );
        assert_eq!(parse_java_version_input("zulu@17").unwrap(), ("17".into(), JavaVendor::Zulu));
        assert_eq!(
            parse_java_version_input("amzn@21.0.13").unwrap(),
            ("21.0.13".into(), JavaVendor::Corretto)
        );
        // tem/temurin 归一化为无后缀 Temurin
        assert_eq!(parse_java_version_input("21-tem").unwrap(), ("21".into(), JavaVendor::Temurin));
        assert_eq!(parse_java_version_input("tem@21").unwrap(), ("21".into(), JavaVendor::Temurin));
        // 未知尾段不拆（保持旧行为）
        assert_eq!(
            parse_java_version_input("1.0.0-beta").unwrap(),
            ("1.0.0-beta".into(), JavaVendor::Temurin)
        );
    }

    #[test]
    fn parse_java_input_rejects_bad_vendor_and_conflicts() {
        assert!(parse_java_version_input("bogus@21").is_err());
        assert!(parse_java_version_input("zulu@").is_err());
        assert!(parse_java_version_input("zulu@21-amzn").is_err());
        // 同供应商后缀与 @ 组合是冗余但合法
        assert_eq!(
            parse_java_version_input("zulu@21-zulu").unwrap(),
            ("21".into(), JavaVendor::Zulu)
        );
    }

    #[test]
    fn vendor_fuzzy_isolates_default_vendor() {
        let versions = vs(&["21.0.13", "21.0.13-zulu", "17.0.9-zulu"]);
        // "21" 只匹配 Temurin，不会被 zulu 劫持
        let m = fuzzy_match_version_with_vendor(&versions, "21").unwrap();
        assert_eq!(m.full_version, "21.0.13");
        // "21-zulu" 与 "zulu@21" 等价
        let m2 = fuzzy_match_version_with_vendor(&versions, "21-zulu").unwrap();
        assert_eq!(m2.full_version, "21.0.13-zulu");
        assert!(m2.fuzzy_matched);
        let m3 = fuzzy_match_version_with_vendor(&versions, "zulu@21").unwrap();
        assert_eq!(m3.full_version, "21.0.13-zulu");
        // "17" 无 Temurin 家族 → 报错并提示已装的 zulu 版本
        let err = fuzzy_match_version_with_vendor(&versions, "17").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("other vendors installed"));
        assert!(msg.contains("try '17.0.9-zulu'"));
    }

    #[test]
    fn vendor_fuzzy_exact_and_prefix() {
        let versions = vs(&["21.0.13", "21.0.13-zulu", "21.0.13.0.11-amzn"]);
        let m = fuzzy_match_version_with_vendor(&versions, "21.0.13-zulu").unwrap();
        assert_eq!(m.full_version, "21.0.13-zulu");
        assert!(!m.fuzzy_matched);
        // corretto tag 是多段数字，前缀 "21.0.13" 能命中
        let m2 = fuzzy_match_version_with_vendor(&versions, "amzn@21.0.13").unwrap();
        assert_eq!(m2.full_version, "21.0.13.0.11-amzn");
        assert!(m2.fuzzy_matched);
    }
}
