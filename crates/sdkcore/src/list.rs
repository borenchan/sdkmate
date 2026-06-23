use crate::manager::SdkManager;
use crate::install::downloader::build_reqwest_client;
use crate::install::progress::InstallProgress;
use crate::install::resolver::{VersionSource, fetch_version_data, get_install_strategy};
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;
use util::consts::STATUS_ACTIVE;
use util::path::get_installed_sdks_dir;
use util::sdk::{BuiltinSdk, Sdk};
use util::sdk_resources::find_builtin_sdk_config;
use util::{divider, info, success, try_bug, warning};

// ─── Data Structures ───────────────────────────────────────────────

#[derive(Debug)]
pub struct SdkVersionItem {
    pub sdk: Sdk,
    pub sdk_version: String,
    pub sdk_dir: PathBuf,
    pub is_active: bool,
}

impl SdkVersionItem {
    pub fn new(sdk: Sdk, sdk_dir: PathBuf, is_active: bool) -> Self {
        let cow = sdk_dir.file_name().unwrap_or(OsStr::new("(empty dir)")).to_string_lossy();
        Self {
            sdk,
            sdk_version: cow.to_string(),
            sdk_dir,
            is_active,
        }
    }
}

/// Remote version entry enriched with local install status
#[derive(Debug, Clone)]
pub struct RemoteVersionItem {
    pub full_version: String,
    pub feature_version: Option<String>,
    pub install_status: InstallStatus,
    /// Source URL for transparency display
    pub source_url: String,
}

/// Install status marker for remote version display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallStatus {
    /// Not installed locally
    NotInstalled,
    /// Installed locally but not the active version
    Installed,
    /// Installed and is the currently active version
    Active,
}

// ─── Local List ────────────────────────────────────────────────────

/// Remote version list result with total count before truncation
pub struct RemoteVersionResult {
    pub items: Vec<RemoteVersionItem>,
    /// Total count before limit truncation (for display in TUI header)
    pub total_count: usize,
}

impl SdkManager {
    /// Print all installed SDKs with current version (non-interactive summary)
    pub fn show_local_sdk_list(&self) -> Result<()> {
        let sdk_dir = get_installed_sdks_dir()?;
        let mut i = 1;
        divider!();
        info!("Installed SDKs:");
        divider!();
        for entry in sdk_dir.read_dir()?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let sdk_name = entry.file_name().to_string_lossy().to_string();
            let current = self
                .match_valid_sdk(&sdk_name)
                .ok()
                .and_then(|sdk| self.config.find_sdk_ok(&sdk).ok())
                .and_then(|conf| conf.current_version.clone())
                .unwrap_or_else(|| "N/A".to_string());
            success!("{i:>2}. {:<10} current: {}", sdk_name, current);
            i += 1;
        }
        divider!();
        Ok(())
    }

    /// Show current version for one or all SDKs (used by `sdkm current` command)
    pub fn show_local_sdks_current(&self, sdk: Option<Sdk>) -> Result<()> {
        if let Some(sdk) = sdk {
            let conf = self.config.find_sdk_ok(&sdk)?;
            info!("{} {}", sdk, conf.current_version.clone().unwrap_or("N/A".to_string()));
            return Ok(());
        }
        for entry in get_installed_sdks_dir()?.read_dir()?.filter_map(|e| e.ok()) {
            let sdk = self.match_valid_sdk(&entry.file_name().to_string_lossy())?;
            let sdk_conf = self.config.find_sdk_ok(&sdk)?;
            divider!();
            success!(
                "{} current is {}",
                sdk,
                &sdk_conf.current_version.clone().unwrap_or("N/A".to_string())
            );
            divider!();
        }
        Ok(())
    }

    /// Get local SDK versions (data layer, for TUI selector)
    pub fn list_local_sdk_versions(&self, sdk: &Sdk) -> Result<Vec<SdkVersionItem>> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let sdks_root_dir = get_installed_sdks_dir()?;
        let sdk_dir = sdks_root_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy() == sdk.to_string());
        if let Some(sdk_dir) = sdk_dir {
            let result = sdk_dir
                .path()
                .read_dir()?
                .filter_map(|entry| entry.ok())
                .map(|sdk_version| {
                    let sdk_version_dir = sdk_version.path();
                    let is_active = sdk_conf
                        .current_version
                        .clone()
                        .is_some_and(|current| current == sdk_version.file_name().to_string_lossy().as_ref());
                    SdkVersionItem::new(sdk.clone(), sdk_version_dir, is_active)
                })
                .collect();
            return Ok(result);
        }
        info!("SDK '{}' not found in store. Please install a version first.", sdk);
        Ok(vec![])
    }

    /// Print local version list with ✅ active marker (fallback for non-TUI)
    pub fn show_local_sdk_version_list(&self, sdk: &Sdk) -> Result<()> {
        let versions = self.list_local_sdk_versions(sdk)?;
        let mut i = 1;
        let prefix = " ".repeat(UnicodeWidthStr::width(STATUS_ACTIVE));
        versions.iter().for_each(|v| {
            info!(
                "{} {:>2}. {}",
                if v.is_active { STATUS_ACTIVE } else { &prefix },
                i,
                v.sdk_version
            );
            i += 1;
        });
        Ok(())
    }

    // ─── Remote List ────────────────────────────────────────────────

    /// Async: fetch remote versions — core logic with spinner
    ///
    /// Reuses resolver's fetch_version_data + parse_version_data pipeline.
    /// Cache-first with TTL ensures instant response on subsequent calls.
    async fn fetch_remote_versions_async(&self, sdk: &Sdk, limit: u32) -> Result<RemoteVersionResult> {
        let sdk_conf = self.config.find_sdk_ok(sdk)?;
        let strategy = get_install_strategy(sdk);
        let client = build_reqwest_client(&self.config.network)?;
        let sdk_name = sdk.to_string();
        let cache_key = sdk_name.to_lowercase();

        // Maven has no version_url → remote list not supported
        if let Sdk::Built(BuiltinSdk::Maven) = sdk {
            bail!("Maven does not support remote version listing. Specify an exact version to install.");
        }

        // Build version source URLs
        let (primary_url, secondary_url, source_display_url) = match sdk {
            Sdk::Built(b) => {
                let cfg = find_builtin_sdk_config(b).context(format!("no builtin config for {}", sdk_name))?;
                // 内置配置缺失属于程序 bug，标记 BugReportError
                (
                    cfg.version_url.to_string(),
                    cfg.version_fallback_url.map(|s| s.to_string()),
                    if !cfg.version_url.is_empty() {
                        cfg.version_url.to_string()
                    } else {
                        "N/A".to_string()
                    },
                )
            }
            Sdk::Custom(_) => {
                let url = sdk_conf.version_url.clone().unwrap_or_default();
                (
                    url.clone(),
                    sdk_conf.version_fallback_url.clone(),
                    if !url.is_empty() { url } else { "N/A".to_string() },
                )
            }
        };

        // No version discovery source → cannot list remote versions
        if primary_url.is_empty() && secondary_url.as_ref().is_none_or(|s| s.is_empty()) {
            bail!("{} has no version_url configured, cannot list remote versions", sdk_name);
        }

        // Show spinner while fetching
        let pb = InstallProgress::new_resolve(&sdk_name, "");
        let source = VersionSource {
            primary_url,
            secondary_url,
        };

        // Python fallback is GitHub API → needs Accept header
        let headers = if let Sdk::Built(BuiltinSdk::Python) = sdk {
            Some(HashMap::from([(
                "Accept".to_string(),
                "application/vnd.github+json".to_string(),
            )]))
        } else {
            None
        };

        // Reuse resolver's cache-first + fetch pipeline
        let body = fetch_version_data(
            &client,
            &source,
            &cache_key,
            &sdk_name,
            headers,
            self.config.network.cache_ttl_secs as u64,
        )
        .await?;
        pb.finish_with_message(format!("✅ Fetched remote versions for {}", sdk_name));

        let entries = strategy.parse_version_data(&body)?;

        // Enrich with local install status
        let local_versions = self.list_local_sdk_versions(sdk)?;
        let current_version = sdk_conf.current_version.as_deref();

        let items: Vec<RemoteVersionItem> = entries
            .iter()
            .map(|entry| {
                let is_local = local_versions.iter().any(|v| v.sdk_version == entry.full_version);
                let is_active = current_version == Some(&entry.full_version);
                let status = if is_active {
                    InstallStatus::Active
                } else if is_local {
                    InstallStatus::Installed
                } else {
                    InstallStatus::NotInstalled
                };
                RemoteVersionItem {
                    full_version: entry.full_version.clone(),
                    feature_version: entry.feature_version.clone(),
                    install_status: status,
                    source_url: source_display_url.to_string(),
                }
            })
            .collect();

        // Apply limit
        let max = limit as usize;
        let total_count = items.len();
        let result_items = if items.len() > max {
            warning!("{} has {} remote versions, showing first {}", sdk_name, items.len(), max);
            items[..max].to_vec()
        } else {
            items
        };

        Ok(RemoteVersionResult {
            items: result_items,
            total_count,
        })
    }

    /// Sync bridge: fetch remote version list data (for TUI selector)
    pub fn fetch_remote_version_list(&self, sdk: &Sdk, limit: u32) -> Result<RemoteVersionResult> {
        let rt = try_bug!(tokio::runtime::Runtime::new().context("Failed to create tokio runtime"));
        rt.block_on(self.fetch_remote_versions_async(sdk, limit))
    }
}
