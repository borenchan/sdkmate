// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境 + 删除 home 目录内容
//
// 复用 cleanup_sdk_environment（清单个 SDK 的 symlink/PATH/env/current）逐个清理激活的 SDK。
// binary 本身不自删（Windows 上 running exe 被锁不可靠），提示用户手动删除。
//
// 交互确认由 CLI 层（SelfUninstallHandler）负责，core 只做业务，便于测试直接调用无需应答 stdin。

use crate::env::report_env_actions;
use crate::manager::SdkManager;
use anyhow::Result;
use sdkm_util::consts::{CONFIG_FILE_NAME, SDKM_CACHE_DIR, SDKM_LINKS_DIR, SDKM_STORE_DIR, SDKM_TMP_DIR};
use sdkm_util::path::get_sdkm_home;
use sdkm_util::sdk::Sdk;
use std::env;
use std::fs;
use std::str::FromStr;

impl SdkManager {
    /// 卸载 sdkm 自身：清理所有被管理 SDK 的激活环境并删除 home 目录内容
    ///
    /// - 遍历 config 中所有激活的 SDK，复用 `cleanup_sdk_environment` 清其 symlink/PATH/env/current
    /// - 删除 home 目录内容（store/links/.tmp/cache/config.toml），尽力而为
    /// - binary 本身与 PATH 条目提示用户手动清理（running exe 自删跨平台不可靠）
    ///
    /// 确认提示由 CLI 层负责（破坏性操作必须交互确认），core 不含交互逻辑。
    pub fn uninstall_self(&mut self) -> Result<()> {
        // 收集激活的 SDK（owned，避免借 self.config 与后续 &mut self 冲突）
        let active_sdks: Vec<Sdk> = self
            .config
            .sdks
            .iter()
            .filter(|c| c.current_version.is_some())
            .filter_map(|c| Sdk::from_str(&c.name).ok())
            .collect();

        self.sink
            .info(format!("cleaning up {} managed SDK environment(s)...", active_sdks.len()));
        for sdk in &active_sdks {
            // 复用 cleanup_sdk_environment：删 symlink + remove PATH + unset env + current=None
            if let Err(e) = self.cleanup_sdk_environment(sdk) {
                self.sink.warning(format!("failed to clean up `{sdk}` environment: {e}"));
            }
        }

        // hook 行：当前 shell 精确移除（固定注释行 + hook 调用行 + legacy 形态）。
        // 走 env_operation：测试用 MockEnv 默认空操作，绝不触碰真实 profile
        if let Err(e) = self.env_operation.remove_shell_hook(self.sink.as_ref()) {
            self.sink.warning(format!("failed to remove shell hook lines: {e}"));
        }

        // 旧版直写 profile 的 PATH/export 行精准清扫（仅删 sdkm 管辖路径的行，宁可漏删不误删）。
        // links 根与 extra_vars 键名必须在删除 config.toml 之前从配置解析。
        let links_root = self.config.resolved_symlink_dir().unwrap_or_default();
        let env_keys: Vec<String> = self.config.sdks.iter().flat_map(|s| s.extra_vars.keys().cloned()).collect();
        let home_str = get_sdkm_home()?.to_string_lossy().to_string();
        match self
            .env_operation
            .cleanup_legacy_profile_lines(&home_str, &links_root, &env_keys)
        {
            Err(e) => self.sink.warning(format!("failed to sweep legacy profile lines: {e}")),
            Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
        }

        // 托管环境文件 + profile source 行：必须在删除 config.toml 之前
        // （自定义 env_file 路径靠 config 解析，删了 config 就只能回退默认路径）
        match self.env_operation.cleanup_env_file() {
            Err(e) => self.sink.warning(format!("failed to clean up env file: {e}")),
            Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
        }

        // 删除 home 目录内容（store/links/.tmp/cache/config.toml），尽力而为
        let home = get_sdkm_home()?;
        self.sink.info(format!("removing sdkm home contents at {}", home.display()));
        for sub in [SDKM_STORE_DIR, SDKM_LINKS_DIR, SDKM_TMP_DIR, SDKM_CACHE_DIR] {
            let p = home.join(sub);
            if p.exists()
                && let Err(e) = fs::remove_dir_all(&p)
            {
                self.sink.warning(format!("failed to remove `{}`: {}", p.display(), e));
            }
        }
        let cfg = home.join(CONFIG_FILE_NAME);
        if cfg.exists()
            && let Err(e) = fs::remove_file(&cfg)
        {
            self.sink.warning(format!("failed to remove `{}`: {}", cfg.display(), e));
        }

        // sdkm 自身的 PATH 条目：自动移除（binary 保留但 PATH 条目应清理，尽力而为）
        // sdkm 目录可能不在 PATH 中（remove_sdk_path 幂等：无该条目则静默）
        if let Ok(exe) = env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let exe_dir_str = exe_dir.to_string_lossy().to_string();
                match self.env_operation.remove_sdk_path(&exe_dir_str) {
                    Err(e) => {
                        self.sink
                            .warning(format!("failed to remove sdkm PATH entry `{}`: {}", exe_dir_str, e));
                    }
                    Ok(actions) => report_env_actions(self.sink.as_ref(), &actions),
                }
            }
            // binary 本身：running exe 跨平台不可靠自删（Windows 锁），提示用户手动删
            self.sink.warning(format!(
                "sdkm binary remains at `{}` (cannot self-remove while running); please remove it manually",
                exe.display()
            ));
        }

        self.sink
            .success("sdkm self-uninstall complete (binary remains to remove manually)".to_string());
        Ok(())
    }
}
