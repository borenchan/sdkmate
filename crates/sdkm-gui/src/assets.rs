//! GUI 资产源：组件默认图标集 + 应用实际使用的额外图标 + 品牌图片（全部嵌入）
//!
//! 不再回退 AllAssets（1830 个 Lucide 全量目录）；只嵌入用到的额外图标，
//! 组件自身需要的图标由 gpui-kit 的默认 `Assets`（106 个）覆盖，减小二进制体积。

use gpui_kit::assets::Assets;
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;
#[cfg(not(windows))]
use std::sync::Arc;

/// 应用使用、但不在组件默认 106 图标集内的额外图标
const EXTRA_ICONS: &[(&str, &[u8])] = &[
    ("icons/activity.svg", include_bytes!("../assets/icons/activity.svg")),
    ("icons/cloud-off.svg", include_bytes!("../assets/icons/cloud-off.svg")),
    ("icons/coffee.svg", include_bytes!("../assets/icons/coffee.svg")),
    ("icons/container.svg", include_bytes!("../assets/icons/container.svg")),
    ("icons/download.svg", include_bytes!("../assets/icons/download.svg")),
    ("icons/feather.svg", include_bytes!("../assets/icons/feather.svg")),
    ("icons/file-code.svg", include_bytes!("../assets/icons/file-code.svg")),
    ("icons/git-branch.svg", include_bytes!("../assets/icons/git-branch.svg")),
    ("icons/hexagon.svg", include_bytes!("../assets/icons/hexagon.svg")),
    ("icons/house.svg", include_bytes!("../assets/icons/house.svg")),
    ("icons/layers.svg", include_bytes!("../assets/icons/layers.svg")),
    ("icons/monitor.svg", include_bytes!("../assets/icons/monitor.svg")),
    (
        "icons/mouse-pointer-click.svg",
        include_bytes!("../assets/icons/mouse-pointer-click.svg"),
    ),
    ("icons/package-open.svg", include_bytes!("../assets/icons/package-open.svg")),
    ("icons/package.svg", include_bytes!("../assets/icons/package.svg")),
    ("icons/pencil.svg", include_bytes!("../assets/icons/pencil.svg")),
    ("icons/puzzle.svg", include_bytes!("../assets/icons/puzzle.svg")),
    ("icons/save.svg", include_bytes!("../assets/icons/save.svg")),
    ("icons/search-x.svg", include_bytes!("../assets/icons/search-x.svg")),
    ("icons/shield-check.svg", include_bytes!("../assets/icons/shield-check.svg")),
    ("icons/trash.svg", include_bytes!("../assets/icons/trash.svg")),
    ("icons/zap.svg", include_bytes!("../assets/icons/zap.svg")),
];

/// sdkm GUI 资产源
pub struct SdkAssets;

impl AssetSource for SdkAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "app-icon.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/gui-icons/02-app-icon-simplified-128.png"
            )))),
            _ => {
                if let Some((_, data)) = EXTRA_ICONS.iter().find(|(p, _)| *p == path) {
                    Ok(Some(Cow::Borrowed(*data)))
                } else {
                    Assets.load(path)
                }
            }
        }
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

/// 窗口/任务栏图标（非 Windows；Windows 由 exe 资源 ID 1 提供）
#[cfg(not(windows))]
pub fn window_icon() -> Option<Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../../../assets/gui-icons/02-app-icon-simplified-256.png");
    image::load_from_memory(bytes).ok().map(|img| Arc::new(img.to_rgba8()))
}
