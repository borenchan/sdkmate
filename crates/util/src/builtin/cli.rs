// CLI 体验类种子：命令行效率工具（查找/替换/预览/装饰）

use super::SdkSeed;

pub const FZF: SdkSeed = SdkSeed::gh("fzf", "https://api.github.com/repos/junegunn/fzf/releases", &["fzf"]);
// 资产前缀 ripgrep 与主命令 rg 不同
pub const RIPGREP: SdkSeed =
    SdkSeed::gh("ripgrep", "https://api.github.com/repos/BurntSushi/ripgrep/releases", &["rg"]).prefix("ripgrep");
pub const FD: SdkSeed = SdkSeed::gh("fd", "https://api.github.com/repos/sharkdp/fd/releases", &["fd"]);
pub const BAT: SdkSeed = SdkSeed::gh("bat", "https://api.github.com/repos/sharkdp/bat/releases", &["bat"]);
pub const EZA: SdkSeed = SdkSeed::gh("eza", "https://api.github.com/repos/eza-community/eza/releases", &["eza"]);
pub const DELTA: SdkSeed = SdkSeed::gh("delta", "https://api.github.com/repos/dandavison/delta/releases", &["delta"]);
