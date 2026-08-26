use clap::Parser;
use cli::SdkMateCli;
#[cfg(debug_assertions)]
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    // debug 构建自动启用 backtrace（anyhow 需要此环境变量才能捕获堆栈）
    // Rust 2024 edition 中 set_var 是 unsafe，这里仅在程序启动最早期设置，
    // 不存在多线程竞争风险
    #[cfg(debug_assertions)]
    if env::var("RUST_BACKTRACE").is_err() {
        unsafe {
            env::set_var("RUST_BACKTRACE", "1");
        }
    }

    let cli = SdkMateCli::parse();
    cli.run()
}
