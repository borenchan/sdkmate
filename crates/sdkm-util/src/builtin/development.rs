// 开发工具链：官方可直接解压的归档走精确版本安装；组件式发行保留为内置本地 SDK。

use super::SdkSeed;

// 完整 Android SDK 根目录；command-line tools 单包不等于完整 SDK。
pub const ANDROID: SdkSeed = SdkSeed::local("android", &["adb", "sdkmanager", "avdmanager"])
    .bin("platform-tools")
    .vars(&[("ANDROID_HOME", "{sdk_dir}"), ("ANDROID_SDK_ROOT", "{sdk_dir}")])
    .paths(&["cmdline-tools/latest/bin", "emulator"]);

// Dart 各平台均为 zip；官方 macOS 名为 macos，而不是默认的 darwin。
pub const DART: SdkSeed = SdkSeed::archive(
    "dart",
    if cfg!(target_os = "windows") {
        "https://storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-windows-{arch}-release.zip"
    } else if cfg!(target_os = "macos") {
        "https://storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-macos-{arch}-release.zip"
    } else {
        "https://storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-linux-{arch}-release.zip"
    },
    &["dart"],
)
.bin("bin");

// .NET SDK 根目录包含 host/shared/sdk 等组件，不使用 bin/ 子目录。
pub const DOTNET: SdkSeed = SdkSeed::archive(
    "dotnet",
    if cfg!(target_os = "windows") {
        "https://builds.dotnet.microsoft.com/dotnet/Sdk/{version}/dotnet-sdk-{version}-win-{arch}.zip"
    } else if cfg!(target_os = "macos") {
        "https://builds.dotnet.microsoft.com/dotnet/Sdk/{version}/dotnet-sdk-{version}-osx-{arch}.tar.gz"
    } else if cfg!(target_env = "musl") {
        "https://builds.dotnet.microsoft.com/dotnet/Sdk/{version}/dotnet-sdk-{version}-linux-musl-{arch}.tar.gz"
    } else {
        "https://builds.dotnet.microsoft.com/dotnet/Sdk/{version}/dotnet-sdk-{version}-linux-{arch}.tar.gz"
    },
    &["dotnet"],
)
.vars(&[("DOTNET_ROOT", "{sdk_dir}")]);

// Flutter 的缓存、Git 元数据与平台工具链需一起准备，不把源码归档当作已安装 SDK。
pub const FLUTTER: SdkSeed = SdkSeed::local("flutter", &["flutter", "dart"])
    .bin("bin")
    .vars(&[("FLUTTER_ROOT", "{sdk_dir}")]);

// Gradle 为与平台无关的 zip；运行仍需可用的 JDK。
pub const GRADLE: SdkSeed = SdkSeed::archive(
    "gradle",
    "https://services.gradle.org/distributions/gradle-{version}-bin.zip",
    &["gradle"],
)
.bin("bin")
.vars(&[("GRADLE_HOME", "{sdk_dir}")]);

// 官方 LLVM / MinGW 发行格式与安装布局不统一，先管理已准备好的完整工具链。
pub const LLVM: SdkSeed = SdkSeed::local("llvm", &["clang", "clang++", "lld", "lld-link", "llvm-ar"]).bin("bin");
pub const MINGW: SdkSeed = SdkSeed::local("mingw", &["gcc", "g++", "gdb", "mingw32-make"]).bin("bin");

// 必须是已装工具链根（bin/rustc、bin/cargo），不是 .cargo/bin 的 rustup 代理。
pub const RUST: SdkSeed = SdkSeed::local("rust", &["rustc", "cargo", "rustdoc", "rustfmt"]).bin("bin");
