// 安全类种子：加密/密钥管理工具

use super::SdkSeed;

// zip 内 age/ 提升后 age.exe + age-keygen.exe 双命令在根
pub const AGE: SdkSeed = SdkSeed::gh(
    "age",
    "https://api.github.com/repos/FiloSottile/age/releases",
    &["age", "age-keygen"],
    None,
    None,
);
