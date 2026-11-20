// ──────────────────────────────────────────────────────
// Unix 环境操作：写入 sdkm 托管环境文件（<home>/sdkm_env.sh | sdkm_env.fish，
// 或 config 的 env_file 自定义路径），profile 只 source 该文件（见 shell/inject.rs）。
//
// 持久化语法全部来自 sdkm_util::shell_backend::ProfilePersistence：
//   bash/zsh → RebuildLine（`export PATH="a:b:$PATH"` 单行整体重建）
//   fish     → PerDirCommand（每目录一行 `fish_add_path --path "<dir>"`）
// PowerShell 无 unix 持久化（走 Windows 注册表），detect_shell 在 unix 不出 PowerShell。
//
// 文件内用 ENV_BLOCK_BEGIN/END 标记包住 sdkm 托管块：自定义 env_file 指向用户自有
// 文件（如 ~/.devrc）时只改块内内容，块外原样保留。
// ──────────────────────────────────────────────────────

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::slice;

use anyhow::{Context, Result};
use sdkm_util::consts::{ENV_BLOCK_BEGIN, ENV_BLOCK_END, ENV_PATH, ENV_SOURCE_COMMENT_LINE};
use sdkm_util::path::{expand_tilde, get_default_env_file_path};
use sdkm_util::shell::{PathModel, ProfilePersistence, Shell, detect_shell};

use crate::config::SdkmConfig;
use crate::env::{EnvAction, EnvOperation};
use crate::report::EventSink;
use crate::shell::inject;

/// RebuildLine 模型（bash/zsh）的 PATH 行行首前缀——两者共用同一 `export PATH=` 形态
const REBUILD_PATH_PREFIX: &str = "export PATH=";

/// fish 的逐目录 PATH 命令行前缀（清扫时精确识别 sdkm 写入形态）
const FISH_ADD_PATH_PREFIX: &str = "fish_add_path --path \"";

/// 路径是否属于 sdkm 管辖：等于 home / 等于 links 根 / 位于 links 根目录树内。
/// 两侧归一化尾部 `/` 再比较（历史写入存在 `links/<sdk>/` 带尾斜杠形态）
fn is_sdkm_path(entry: &str, home: &str, links_root: &str) -> bool {
    fn norm(s: &str) -> &str {
        s.trim_end_matches('/')
    }
    let (e, h, l) = (norm(entry), norm(home), norm(links_root));
    if h.is_empty() || l.is_empty() {
        return false;
    }
    e == h || e == l || e.starts_with(&format!("{l}/"))
}

/// 清扫单个 profile 里旧版直写的 sdkm 行，返回移除的行数（0 = 无需写回）
///
/// 精准匹配（宁可漏删不误删）：
/// - RebuildLine（bash/zsh）：`export PATH=` 行内只过滤值属于 sdkm 管辖路径的条目，
///   剩余条目原样重建（保留 backref）；条目被清空则整行删除
/// - PerDirCommand（fish）：`fish_add_path --path "<dir>"` 行仅在 dir 属于 sdkm 管辖时删除
/// - 赋值行：仅 `env_keys` 中的变量名且值属于 sdkm 管辖路径时删除
fn sweep_profile_file(
    file_path: &Path,
    p: &'static ProfilePersistence,
    home: &str,
    links_root: &str,
    env_keys: &[String],
) -> Result<usize> {
    let content = fs::read_to_string(file_path).unwrap_or_default();
    if content.is_empty() {
        return Ok(0);
    }
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut removed = 0usize;

    match p.path_model {
        PathModel::RebuildLine => {
            let parse = p.parse_profile_path_line.context("RebuildLine 缺 parse 语法")?;
            let build = p.profile_path_line.context("RebuildLine 缺 build 语法")?;
            if let Some(idx) = lines.iter().position(|l| l.trim().starts_with(REBUILD_PATH_PREFIX)) {
                let (entries, backref) = parse(&lines[idx]);
                let kept: Vec<String> =
                    entries.iter().filter(|e| !is_sdkm_path(e, home, links_root)).cloned().collect();
                if kept.len() != entries.len() {
                    removed += 1;
                    if kept.is_empty() {
                        // 行内只剩 sdkm 条目：整行删除（系统 PATH 由 /etc/profile 等提供）
                        lines.remove(idx);
                    } else {
                        lines[idx] = build(&kept, backref);
                    }
                }
            }
        }
        PathModel::PerDirCommand => {
            let before = lines.len();
            lines.retain(|l| {
                let t = l.trim();
                let Some(dir) = t.strip_prefix(FISH_ADD_PATH_PREFIX).and_then(|r| r.strip_suffix('"')) else {
                    return true;
                };
                !is_sdkm_path(dir, home, links_root)
            });
            removed += before - lines.len();
        }
    }

    // extra_vars 赋值行：变量名在白名单 + 值属于 sdkm 管辖路径
    for key in env_keys {
        let export_prefix = (p.export_prefix)(key);
        let before = lines.len();
        lines.retain(|l| {
            let t = l.trim();
            if !t.starts_with(&export_prefix) {
                return true;
            }
            let value = t.strip_prefix(&export_prefix).unwrap_or("").trim_matches('"');
            !is_sdkm_path(value, home, links_root)
        });
        removed += before - lines.len();
    }

    if removed > 0 {
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        fs::write(file_path, text)?;
    }
    Ok(removed)
}

/// 块内文件头注释（首次创建块时写入）
const BLOCK_HEADER: &str = "# sdkm environment - managed by sdkm, do not edit manually.";
/// 块内 PATH 分节注释（新增 PATH 行插在其后）
const SECTION_PATH: &str = "# PATH additions";
/// 块内环境变量分节注释（新增 export 行插在其后）
const SECTION_ENV: &str = "# SDK environment variables";

/// 托管环境文件路径：config.env_file 优先（展开 `~`），否则跟随 home（`<home>/sdkm_env.sh|fish`）。
///
/// 不依赖 SdkManager 持有的 config（init 首次运行时尚无 config.toml），
/// 读取失败/未配置一律回退默认路径。默认文件名按检测到的 shell（fish → .fish）。
pub fn env_file_path() -> Result<PathBuf> {
    let shell = detect_shell();
    let custom = SdkmConfig::read_from_disk().ok().and_then(|c| c.env_file);
    match custom {
        Some(p) if !p.trim().is_empty() => Ok(PathBuf::from(expand_tilde(p.trim()))),
        _ => get_default_env_file_path(shell == Shell::Fish),
    }
}

/// 迁移托管块内容到新环境文件（`config set/delete env_file` 用）。
///
/// 旧文件保持不动（其 source 行由重跑 init 时替换）；新文件已有托管内容时不覆盖。
pub fn migrate_env_file(old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    let (_, old_block, _) = UnixEnvOperation::read_doc(Path::new(old));
    if !UnixEnvOperation::has_block_content(&old_block) {
        return Ok(());
    }
    let (before, new_block, after) = UnixEnvOperation::read_doc(Path::new(new));
    if UnixEnvOperation::has_block_content(&new_block) {
        return Ok(());
    }
    UnixEnvOperation::write_doc(Path::new(new), &before, &old_block, &after)
}

/// Unix 环境操作（shell 每次操作按 `$SHELL` 自动检测）
pub struct UnixEnvOperation {}

impl UnixEnvOperation {
    /// 当前 shell 的持久化表（unix 下必为 bash/zsh/fish，均有持久化）
    fn current_persistence() -> Result<&'static ProfilePersistence> {
        detect_shell()
            .persistence()
            .context("current shell has no unix profile persistence")
    }

    /// 读环境文件并切出托管块：(块前内容, 块内行, 块后内容)。
    /// 文件不存在/无块标记 → 块为空、全文视作块前内容（写入时块追加到末尾）。
    fn read_doc(file_path: &Path) -> (Vec<String>, Vec<String>, Vec<String>) {
        let content = fs::read_to_string(file_path).unwrap_or_default();
        let lines: Vec<String> = content.lines().map(String::from).collect();
        let Some(begin) = lines.iter().position(|l| l.trim() == ENV_BLOCK_BEGIN) else {
            return (lines, Vec::new(), Vec::new());
        };
        match lines[begin + 1..].iter().position(|l| l.trim() == ENV_BLOCK_END) {
            Some(offset) => {
                let end = begin + 1 + offset;
                (
                    lines[..begin].to_vec(),
                    lines[begin + 1..end].to_vec(),
                    lines[end + 1..].to_vec(),
                )
            }
            // 块尾标记被误删：BEGIN 到 EOF 全当块内容，写回时自愈补 END
            None => (lines[..begin].to_vec(), lines[begin + 1..].to_vec(), Vec::new()),
        }
    }

    /// 重写环境文件：块前内容 + 块标记 + 块 + 块后内容。
    /// 块内无实际内容（只剩注释/空行）时不写标记，避免残留空块。
    fn write_doc(file_path: &Path, before: &[String], block: &[String], after: &[String]) -> Result<()> {
        let with_block = Self::has_block_content(block);
        let mut lines: Vec<String> = Vec::with_capacity(before.len() + block.len() + after.len() + 4);
        lines.extend_from_slice(before);
        if with_block {
            // 块前用户内容与块标记之间留空行（不紧贴用户最后一行）
            if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(ENV_BLOCK_BEGIN.to_string());
            lines.extend_from_slice(block);
            lines.push(ENV_BLOCK_END.to_string());
        }
        if !after.is_empty() {
            if with_block {
                lines.push(String::new());
            }
            lines.extend_from_slice(after);
        }
        let mut content = lines.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        if let Some(parent) = file_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }

    /// 块内是否有实际内容（非空且非注释行）
    fn has_block_content(block: &[String]) -> bool {
        block.iter().any(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
    }

    /// 块为空时初始化文件头 + 分节注释（首次写入 / 块被清空后重建）
    fn ensure_block(block: &mut Vec<String>) {
        if block.is_empty() {
            block.push(BLOCK_HEADER.to_string());
            block.push(String::new());
            block.push(SECTION_PATH.to_string());
            block.push(String::new());
            block.push(SECTION_ENV.to_string());
        }
    }

    /// 在分节注释后插入行（跳过紧跟的空行；分节缺失时追加到块尾）
    fn insert_after_section(block: &mut Vec<String>, section: &str, line: String) {
        match block.iter().position(|l| l.trim() == section) {
            Some(i) => {
                let mut at = i + 1;
                while at < block.len() && block[at].trim().is_empty() {
                    at += 1;
                }
                block.insert(at, line);
            }
            None => block.push(line),
        }
    }

    /// 在 profile 行里找 `export PATH=` 行的索引（仅作用于托管块内行）
    fn find_path_export_line(lines: &[String]) -> Option<usize> {
        lines.iter().position(|l| l.trim().starts_with(REBUILD_PATH_PREFIX))
    }

    /// 设置/替换一个赋值行（已存在则替换，无则插入到环境变量分节）。
    /// 行形态走 backend（bash `export K="v"` / fish `set -gx K "v"`）
    fn append_or_replace_export(
        file_path: &Path,
        p: &'static ProfilePersistence,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let expanded_value = expand_tilde(value);
        // 赋值行来自 ShellSyntax（export_line 是脚本语法，persistence 表只提供匹配前缀 export_prefix）
        let new_line = (p.shell.syntax().export_line)(key, &expanded_value);
        let export_prefix = (p.export_prefix)(key);
        let (before, mut block, after) = Self::read_doc(file_path);
        Self::ensure_block(&mut block);

        let mut found = false;
        for line in block.iter_mut() {
            if line.trim().starts_with(&export_prefix) {
                *line = new_line.clone();
                found = true;
                break;
            }
        }
        if !found {
            Self::insert_after_section(&mut block, SECTION_ENV, new_line);
        }
        Self::write_doc(file_path, &before, &block, &after)
    }

    /// 把 new_path 前置到托管块内的 PATH（按 path_model 分发）
    ///
    /// RebuildLine（bash/zsh）：解析块内 PATH 行 → 前置插入 → 整行重建（已存在则跳过）。
    /// PerDirCommand（fish）：`fish_add_path --path "<dir>"` 逐行模型——已存在则跳过，新行插到
    /// PATH 分节；重复 switch 不来回翻转行序（幂等）。
    /// 返回动作清单：已存在分支报 PathExists，实际写入分支返回空（PathAdded 由调用方统一附加）
    fn add_path_entry(file_path: &Path, p: &'static ProfilePersistence, new_path: &str) -> Result<Vec<EnvAction>> {
        let expanded = expand_tilde(new_path);
        let (before, mut block, after) = Self::read_doc(file_path);
        Self::ensure_block(&mut block);
        match p.path_model {
            PathModel::RebuildLine => {
                let parse = p.parse_profile_path_line.context("RebuildLine 缺 parse 语法")?;
                let build = p.profile_path_line.context("RebuildLine 缺 build 语法")?;

                // 只查托管块自己的 export PATH 行是否已含该条目（不读进程 $PATH）：
                // 持久化语义 = 块里没有就写。读进程 $PATH 会让"目录已在 live PATH 但未持久化"
                // 时误判"已存在"跳过写入，重启后 PATH 丢失。
                let path_line_idx = Self::find_path_export_line(&block);
                let already_written =
                    path_line_idx.is_some_and(|idx| parse(&block[idx]).0.iter().any(|e| e == &expanded));
                if already_written {
                    return Ok(vec![EnvAction::PathExists {
                        path: new_path.to_string(),
                    }]);
                }

                match path_line_idx {
                    // 已有 PATH 行：前置插入，保留原 backref 状态（有 $PATH 引用则重建时附回）
                    Some(idx) => {
                        let (entries, backref) = parse(&block[idx]);
                        let mut merged = vec![expanded.clone()];
                        merged.extend(entries);
                        block[idx] = build(&merged, backref);
                    }
                    // 无 PATH 行：新建到 PATH 分节，backref=$PATH（避免冲掉系统 PATH）
                    None => {
                        let new_line = build(slice::from_ref(&expanded), true);
                        Self::insert_after_section(&mut block, SECTION_PATH, new_line);
                    }
                }
                Self::write_doc(file_path, &before, &block, &after)?;
                Ok(Vec::new())
            }
            PathModel::PerDirCommand => {
                let add = p.add_dir_command.context("PerDirCommand 缺 add_dir_command")?;
                let line = add(&expanded);
                if block.iter().any(|l| l.trim() == line) {
                    return Ok(vec![EnvAction::PathExists {
                        path: new_path.to_string(),
                    }]);
                }
                Self::insert_after_section(&mut block, SECTION_PATH, line);
                Self::write_doc(file_path, &before, &block, &after)?;
                Ok(Vec::new())
            }
        }
    }

    /// source 托管环境文件，把 PATH 写回当前进程（只执行该文件，不再跑整个 profile）
    ///
    /// shell 命令与「输出 PATH 的协议」来自 backend：bash/zsh `echo $PATH`（冒号分隔串）；
    /// fish 的 `$PATH` 是 list，`echo` 会每路径一行 → 必须用 `string join : $PATH` 转冒号串。
    fn source_env_file(file_path: &Path) -> Result<()> {
        let p = Self::current_persistence()?;
        let source_cmd = format!("source '{}' 2>/dev/null; {}", file_path.display(), (p.echo_path_cmd)());
        let output = Command::new(p.shell_command).args(["-c", &source_cmd]).output()?;

        if output.status.success() {
            let new_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !new_path.is_empty() {
                // 2024 edition: env::set_var 已标 unsafe（多线程 UB 风险）。
                // sdkm 单线程 CLI 场景功能上生效；彻底修复需重构为 shim 模式（不写当前进程）。
                unsafe {
                    env::set_var(ENV_PATH, new_path);
                }
            }
        }

        Ok(())
    }
}

impl EnvOperation for UnixEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> Result<Vec<EnvAction>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let mut actions = Vec::with_capacity(envs.len());
        for (env_key, env_val) in envs {
            Self::append_or_replace_export(&env_file, p, env_key, env_val)?;
            actions.push(EnvAction::EnvSet {
                key: env_key.clone(),
                value: env_val.clone(),
            });
        }
        Self::source_env_file(&env_file)?;
        Ok(actions)
    }

    fn add_sdk_path(&self, sdk_path: &str) -> Result<Vec<EnvAction>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let mut actions = Self::add_path_entry(&env_file, p, sdk_path)?;
        actions.push(EnvAction::PathAdded {
            path: sdk_path.to_string(),
        });
        Self::source_env_file(&env_file)?;
        Ok(actions)
    }

    fn get_path(&self) -> Result<String> {
        Ok(env::var(ENV_PATH).unwrap_or_default())
    }

    fn remove_sdk_path(&self, target: &str) -> Result<Vec<EnvAction>> {
        let env_file = env_file_path()?;
        let p = Self::current_persistence()?;
        let expanded_target = expand_tilde(target);

        let (before, mut block, after) = Self::read_doc(&env_file);
        if !Self::has_block_content(&block) {
            return Ok(Vec::new()); // 无托管内容，无需移除
        }

        match p.path_model {
            PathModel::RebuildLine => {
                let parse = p.parse_profile_path_line.context("RebuildLine 缺 parse 语法")?;
                let build = p.profile_path_line.context("RebuildLine 缺 build 语法")?;
                if let Some(idx) = Self::find_path_export_line(&block) {
                    let (entries, backref) = parse(&block[idx]);
                    let filtered: Vec<String> = entries
                        .into_iter()
                        .filter(|e| e != &expanded_target.as_str() && e != target)
                        .collect();
                    // 保留 :$PATH 引用（backref），否则 export PATH="<paths>" 会冲掉系统 PATH
                    if filtered.is_empty() {
                        block.remove(idx);
                    } else {
                        block[idx] = build(&filtered, backref);
                    }
                }
                Self::write_doc(&env_file, &before, &block, &after)?;
            }
            PathModel::PerDirCommand => {
                // fish：精确删掉匹配 add_dir_command(target) 的行（与 add 时写入的字符串对齐）
                let add = p.add_dir_command.context("PerDirCommand 缺 add_dir_command")?;
                let expect = add(&expanded_target);
                block.retain(|l| l.trim() != expect);
                Self::write_doc(&env_file, &before, &block, &after)?;
            }
        }

        Self::source_env_file(&env_file)?;
        Ok(vec![EnvAction::PathRemoved {
            path: target.to_string(),
        }])
    }

    fn get_env_value(&self, key: &str) -> Result<Option<String>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let export_prefix = (p.export_prefix)(key);
        let (_, block, _) = Self::read_doc(&env_file);
        for line in &block {
            let line = line.trim();
            if line.starts_with(&export_prefix) {
                let value = line.trim_start_matches(&export_prefix).trim_matches('"');
                return Ok(Some(value.to_string()));
            }
        }
        // 托管块里没有，回退到进程环境变量
        Ok(env::var(key).ok())
    }

    fn unset_sdk_env(&self, key: &str) -> Result<Vec<EnvAction>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let (before, mut block, after) = Self::read_doc(&env_file);
        if !Self::has_block_content(&block) {
            return Ok(Vec::new()); // 无托管内容，无需移除
        }

        let export_prefix = (p.export_prefix)(key);
        block.retain(|l| !l.trim().starts_with(&export_prefix));

        Self::write_doc(&env_file, &before, &block, &after)?;
        Self::source_env_file(&env_file)?;
        Ok(vec![EnvAction::EnvRemoved { key: key.to_string() }])
    }

    fn restore_sdk_envs(&self, old_envs: &HashMap<String, Option<String>>) -> Result<Vec<EnvAction>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let mut actions = Vec::with_capacity(old_envs.len());
        for (env_key, old_val) in old_envs {
            if let Some(val) = old_val {
                Self::append_or_replace_export(&env_file, p, env_key, val)?;
                actions.push(EnvAction::EnvRestored {
                    key: env_key.clone(),
                    value: val.clone(),
                });
            } else {
                actions.extend(self.unset_sdk_env(env_key)?);
            }
        }
        Self::source_env_file(&env_file)?;
        Ok(actions)
    }

    fn cleanup_env_file(&self) -> Result<Vec<EnvAction>> {
        let p = Self::current_persistence()?;
        let env_file = env_file_path()?;
        let env_file_str = env_file.to_string_lossy().to_string();

        // 1. 从 profile 移除 source 行（精确匹配生成形态，避免误删用户自有的 source 行）
        let source_line = (p.shell.syntax().source_env_line)(&env_file_str);
        for profile in p.shell.profile_paths()? {
            let Ok(content) = fs::read_to_string(&profile) else {
                continue;
            };
            let kept: Vec<String> = content
                .lines()
                .filter(|l| l.trim() != source_line.trim() && l.trim() != ENV_SOURCE_COMMENT_LINE)
                .map(String::from)
                .collect();
            if kept.len() != content.lines().count() {
                let mut text = kept.join("\n");
                if !text.is_empty() {
                    text.push('\n');
                }
                fs::write(&profile, text)?;
            }
        }

        // 2. 移除托管块；文件无剩余内容则删除（自定义路径指向用户自有文件时保留其内容）
        let (before, block, after) = Self::read_doc(&env_file);
        if before.is_empty() && !Self::has_block_content(&block) && after.is_empty() {
            // 文件不存在或本来就没有托管内容；内容为纯空白的文件顺手删除
            if env_file.exists() && fs::read_to_string(&env_file).unwrap_or_default().trim().is_empty() {
                let _ = fs::remove_file(&env_file);
            }
            return Ok(Vec::new());
        }
        Self::write_doc(&env_file, &before, &[], &after)?;
        let remaining = fs::read_to_string(&env_file).unwrap_or_default();
        if remaining.trim().is_empty() {
            let _ = fs::remove_file(&env_file);
        }
        Ok(vec![EnvAction::EnvFileRemoved { path: env_file_str }])
    }

    fn cleanup_legacy_profile_lines(
        &self,
        home: &str,
        links_root: &str,
        env_keys: &[String],
    ) -> Result<Vec<EnvAction>> {
        // 只清当前 shell 的 profile（多 shell 混用属少数场景，不做跨 shell 清理的债）
        let shell = detect_shell();
        let Some(p) = shell.persistence() else {
            return Ok(Vec::new());
        };
        let mut actions = Vec::new();
        for profile in shell.profile_paths().unwrap_or_default() {
            let removed = sweep_profile_file(&profile, p, home, links_root, env_keys)?;
            if removed > 0 {
                actions.push(EnvAction::LegacySwept {
                    path: profile.to_string_lossy().to_string(),
                    removed,
                });
            }
        }
        Ok(actions)
    }

    /// shell profile 的 hook 行移除（self uninstall 用；Unix profile 归本实现管）
    fn remove_shell_hook(&self, sink: &dyn EventSink) -> Result<()> {
        inject::remove_shell_hook(sink)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process;
    use std::sync::Mutex;
    use std::thread;

    /// 序列化本模块内改写进程 $PATH 的测试（set_var 全局竞态）
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// 临时唯一环境文件路径（测试不碰真实 home/profile）
    fn temp_env_file() -> PathBuf {
        let dir = env::temp_dir();
        let name = format!("sdkm_env_test_{}_{:?}.sh", process::id(), thread::current().id());
        dir.join(name)
    }

    /// bash RebuildLine：export 写入 → 读回 → 替换 → 取消
    #[test]
    fn bash_export_write_replace_read() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();

        // 新建写入
        UnixEnvOperation::append_or_replace_export(&path, p, "JAVA_HOME", "/sdk/java").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(ENV_BLOCK_BEGIN));
        assert!(content.contains("# SDK environment variables"));
        assert!(content.contains("export JAVA_HOME=\"/sdk/java\""));

        // 覆盖替换（不新增重复行）
        UnixEnvOperation::append_or_replace_export(&path, p, "JAVA_HOME", "/sdk/java2").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().filter(|l| l.contains("JAVA_HOME")).count(), 1);
        assert!(content.contains("export JAVA_HOME=\"/sdk/java2\""));

        let _ = fs::remove_file(&path);
    }

    /// bash RebuildLine：PATH 行新建 + 二次添加前置插入 + 幂等
    #[test]
    fn bash_rebuild_line_add_path_entry() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();

        UnixEnvOperation::add_path_entry(&path, p, "/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# PATH additions"));
        assert!(content.contains("export PATH=\"/sdk/bin:$PATH\""));

        // 第二个目录前置插入，原条目保留
        UnixEnvOperation::add_path_entry(&path, p, "/sdk2/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("export PATH=\"/sdk2/bin:/sdk/bin:$PATH\""));

        // 重复添加已存在目录 → PathExists，文件不变
        let actions = UnixEnvOperation::add_path_entry(&path, p, "/sdk/bin").unwrap();
        assert!(matches!(actions.as_slice(), [EnvAction::PathExists { .. }]));

        let _ = fs::remove_file(&path);
    }

    /// fish PerDirCommand：add_path_entry 追加 fish_add_path 行（重复添加幂等）
    #[test]
    fn fish_per_dir_add_path_entry() {
        let p = Shell::Fish.persistence().unwrap();
        let path = temp_env_file();
        UnixEnvOperation::add_path_entry(&path, p, "/opt/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("fish_add_path --path \"/opt/sdk/bin\""));
        // 幂等：再添加同目录不产生第二行
        UnixEnvOperation::add_path_entry(&path, p, "/opt/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().filter(|l| l.contains("fish_add_path")).count(), 1);
        let _ = fs::remove_file(&path);
    }

    /// 块外用户内容必须原样保留（自定义 env_file 指向用户自有文件的核心约束）
    #[test]
    fn user_content_outside_block_is_preserved() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();
        fs::write(&path, "# user header\nexport MY_VAR=\"1\"\n").unwrap();

        UnixEnvOperation::add_path_entry(&path, p, "/sdk/bin").unwrap();
        UnixEnvOperation::append_or_replace_export(&path, p, "JAVA_HOME", "/sdk/java").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# user header\nexport MY_VAR=\"1\"\n"));
        assert!(content.contains("export PATH=\"/sdk/bin:$PATH\""));
        assert!(content.contains("export JAVA_HOME=\"/sdk/java\""));

        // 移除托管块后用户内容仍在
        let (before, _block, after) = UnixEnvOperation::read_doc(&path);
        UnixEnvOperation::write_doc(&path, &before, &[], &after).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("export MY_VAR=\"1\""));
        assert!(!content.contains(ENV_BLOCK_BEGIN));
        assert!(!content.contains("JAVA_HOME"));
        let _ = fs::remove_file(&path);
    }

    /// Bug 回归：块内 PATH 已含该目录、进程 live $PATH 也含该目录时，仍按"块内为准"幂等
    /// （触发场景：self-uninstall 删了块内行、同会话 live $PATH 仍残留该目录时重跑 init）
    #[test]
    fn add_path_entry_writes_even_if_dir_in_process_path() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();
        let _g = ENV_MUTEX.lock().unwrap();
        // Drop 守卫：无论测试成功或 panic 都恢复进程 $PATH
        struct PathGuard(String);
        impl Drop for PathGuard {
            fn drop(&mut self) {
                unsafe {
                    env::set_var(ENV_PATH, &self.0);
                }
            }
        }
        let _restore = PathGuard(env::var(ENV_PATH).unwrap_or_default());
        // 进程 live $PATH 含 /sdk/bin（模拟 self-uninstall 删了块内行、live $PATH 仍残留）
        unsafe {
            env::set_var(ENV_PATH, "/sdk/bin:/usr/bin");
        }
        // 环境文件不存在（无 PATH 行）
        UnixEnvOperation::add_path_entry(&path, p, "/sdk/bin").unwrap();
        let content = fs::read_to_string(&path).unwrap_or_default();
        // 必须写入（只查环境文件，不读进程 $PATH）
        assert!(content.contains("export PATH=\"/sdk/bin:$PATH\""));
        let _ = fs::remove_file(&path);
    }

    /// 清扫：bash PATH 行只滤 sdkm 条目、保留用户条目；extra_vars 行按「键名 + 值在 links 树内」删
    #[test]
    fn sweep_bash_profile_mixed_lines() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();
        let home = "/home/u/.sdkm";
        let links = "/home/u/.sdkm/links";
        fs::write(
            &path,
            "export PATH=\"/home/u/.sdkm/links/go/bin:/home/u/.sdkm/links/node/bin:/opt/user/bin:$PATH\"\n\
             export JAVA_HOME=\"/home/u/.sdkm/links/java\"\n\
             export MY_VAR=\"1\"\n",
        )
        .unwrap();
        let removed = sweep_profile_file(&path, p, home, links, &["JAVA_HOME".to_string()]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(removed, 2, "PATH 行重建 + JAVA_HOME 行删除");
        assert_eq!(content, "export PATH=\"/opt/user/bin:$PATH\"\nexport MY_VAR=\"1\"\n");
        let _ = fs::remove_file(&path);
    }

    /// 清扫：bash PATH 行只剩 sdkm 条目时整行删除（系统 PATH 由 /etc/profile 等提供）
    #[test]
    fn sweep_bash_removes_sdkm_only_path_line() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();
        fs::write(&path, "export PATH=\"/home/u/.sdkm:/home/u/.sdkm/links/go/bin:$PATH\"\n").unwrap();
        let removed = sweep_profile_file(&path, p, "/home/u/.sdkm", "/home/u/.sdkm/links", &[]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(content, "", "整行删除后文件应为空");
        let _ = fs::remove_file(&path);
    }

    /// 清扫：fish 逐行删除 sdkm 管辖的 fish_add_path 行（含尾斜杠形态），用户行保留
    #[test]
    fn sweep_fish_profile_lines() {
        let p = Shell::Fish.persistence().unwrap();
        let path = temp_env_file();
        fs::write(
            &path,
            "fish_add_path --path \"/home/u/.sdkm\"\n\
             fish_add_path --path \"/home/u/.sdkm/links/etcd/\"\n\
             fish_add_path --path \"/opt/bin\"\n",
        )
        .unwrap();
        let removed = sweep_profile_file(&path, p, "/home/u/.sdkm", "/home/u/.sdkm/links", &[]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(content, "fish_add_path --path \"/opt/bin\"\n");
        let _ = fs::remove_file(&path);
    }

    /// 清扫：无关内容零改动（用户的 PATH/JAVA_HOME 不指向 sdkm 目录 → 不删、不写文件）
    #[test]
    fn sweep_keeps_unrelated_lines() {
        let p = Shell::Bash.persistence().unwrap();
        let path = temp_env_file();
        let original = "export PATH=\"/usr/local/bin:/usr/bin\"\nexport JAVA_HOME=\"/usr/lib/jvm\"\n";
        fs::write(&path, original).unwrap();
        let removed =
            sweep_profile_file(&path, p, "/home/u/.sdkm", "/home/u/.sdkm/links", &["JAVA_HOME".to_string()]).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(fs::read_to_string(&path).unwrap(), original, "零改动时不应写文件");
        let _ = fs::remove_file(&path);
    }
}
