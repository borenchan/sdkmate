use crate::config::SdkmConfig;
use crate::env::{EnvOperation, OsEnvOperation};
use crate::report::EventSink;
use sdkm_util::path::get_sdkm_config_path;
use sdkm_util::sdk::Sdk;
use std::str::FromStr;
use std::sync::Arc;

pub struct SdkManager {
    pub config: SdkmConfig,
    pub env_operation: Box<dyn EnvOperation>,
    /// 过程事件槽：core 经此通知前端过程消息/进度（CLI 装终端适配器，GUI 可换自定义）
    pub sink: Arc<dyn EventSink>,
}

impl SdkManager {
    /// 显式装配事件槽构造——core 自身零打印，输出形态由前端决定
    pub fn new(sink: Arc<dyn EventSink>) -> anyhow::Result<SdkManager> {
        let config = SdkmConfig::read_from_disk()?;
        Ok(SdkManager {
            config,
            env_operation: Box::new(OsEnvOperation {}),
            sink,
        })
    }

    /// 自卸载专用构造：config 文件已不存在（此前卸载过/未 init）时降级用默认配置，
    /// 让 profile 残留清理与 home 目录删除仍可执行；config 存在但损坏则原样报错
    /// （解析错误不该被静默吞掉——按残缺配置清理有误删风险）
    pub fn new_for_uninstall(sink: Arc<dyn EventSink>) -> anyhow::Result<SdkManager> {
        let config = match SdkmConfig::read_from_disk() {
            Ok(c) => c,
            Err(_) if !get_sdkm_config_path()?.exists() => SdkmConfig::default(),
            Err(e) => return Err(e),
        };
        Ok(SdkManager {
            config,
            env_operation: Box::new(OsEnvOperation {}),
            sink,
        })
    }
    /// 校验 SDK 名称是否在配置中注册
    pub fn match_valid_sdk(&self, sdk_name: &str) -> anyhow::Result<Sdk> {
        let sdk = Sdk::from_str(sdk_name)?;
        if !self.config.exist_sdk(&sdk) {
            anyhow::bail!(
                "Unregistered SDK `{}`. Register it with: `sdkm config add-sdk {}` \
                 (add `--download-url <url>` for remote install, or omit for a local switch-only SDK).",
                sdk,
                sdk
            )
        }
        Ok(sdk)
    }
}
