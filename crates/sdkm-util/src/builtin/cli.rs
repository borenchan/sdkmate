// 开发辅助 CLI：单二进制或带少量资源的压缩包分发，全部走标准 B（GitHub Releases 直链）。

use super::SdkSeed;

// 资产前缀 ripgrep（默认取 SDK 名）与主命令 rg 不同
pub const RIPGREP: SdkSeed =
    SdkSeed::gh("ripgrep", "https://api.github.com/repos/BurntSushi/ripgrep/releases", &["rg"]);
pub const FZF: SdkSeed = SdkSeed::gh("fzf", "https://api.github.com/repos/junegunn/fzf/releases", &["fzf"]);
pub const LAZYGIT: SdkSeed = SdkSeed::gh(
    "lazygit",
    "https://api.github.com/repos/jesseduffield/lazygit/releases",
    &["lazygit"],
);
// zip 内 bin/gh.exe，顶层单目录提升后 bin/ 保留
pub const GH: SdkSeed = SdkSeed::gh("gh", "https://api.github.com/repos/cli/cli/releases", &["gh"]).bin("bin");
pub const K9S: SdkSeed = SdkSeed::gh("k9s", "https://api.github.com/repos/derailed/k9s/releases", &["k9s"]);
// 资产前缀 claude 与 SDK 名 claude-code 不同
pub const CLAUDE_CODE: SdkSeed = SdkSeed::gh_prefixed(
    "claude-code",
    "https://api.github.com/repos/anthropics/claude-code/releases",
    "claude",
    &["claude"],
);
pub const FD: SdkSeed = SdkSeed::gh("fd", "https://api.github.com/repos/sharkdp/fd/releases", &["fd"]);
pub const BAT: SdkSeed = SdkSeed::gh("bat", "https://api.github.com/repos/sharkdp/bat/releases", &["bat"]);
pub const EZA: SdkSeed = SdkSeed::gh("eza", "https://api.github.com/repos/eza-community/eza/releases", &["eza"]);
pub const DELTA: SdkSeed = SdkSeed::gh("delta", "https://api.github.com/repos/dandavison/delta/releases", &["delta"]);
pub const STARSHIP: SdkSeed = SdkSeed::gh(
    "starship",
    "https://api.github.com/repos/starship/starship/releases",
    &["starship"],
);
pub const LAZYDOCKER: SdkSeed = SdkSeed::gh(
    "lazydocker",
    "https://api.github.com/repos/jesseduffield/lazydocker/releases",
    &["lazydocker"],
);
pub const STERN: SdkSeed = SdkSeed::gh("stern", "https://api.github.com/repos/stern/stern/releases", &["stern"]);
pub const DIVE: SdkSeed = SdkSeed::gh("dive", "https://api.github.com/repos/wagoodman/dive/releases", &["dive"]);
pub const GRPCURL: SdkSeed = SdkSeed::gh(
    "grpcurl",
    "https://api.github.com/repos/fullstorydev/grpcurl/releases",
    &["grpcurl"],
);
// 资产以 temporal_* 命名（主命令 temporal-server），资产前缀默认取 SDK 名即可命中
pub const TEMPORAL: SdkSeed = SdkSeed::gh(
    "temporal",
    "https://api.github.com/repos/temporalio/temporal/releases",
    &["temporal-server"],
);
// zip 内 age/ 提升后 age.exe + age-keygen.exe 双命令在根
pub const AGE: SdkSeed = SdkSeed::gh(
    "age",
    "https://api.github.com/repos/FiloSottile/age/releases",
    &["age", "age-keygen"],
);

/// 开发辅助 CLI 分类的全部种子（新增命令行工具在这里追加）
pub static SEEDS: &[&SdkSeed] = &[
    &RIPGREP,
    &FZF,
    &LAZYGIT,
    &GH,
    &K9S,
    &CLAUDE_CODE,
    &FD,
    &BAT,
    &EZA,
    &DELTA,
    &STARSHIP,
    &LAZYDOCKER,
    &STERN,
    &DIVE,
    &GRPCURL,
    &TEMPORAL,
    &AGE,
];
