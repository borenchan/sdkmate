use util::path;
use util::sdk::{BuiltinSdk, Sdk};

#[test]
fn test_get_exe_path() {
    let path = path::get_sdkm_home().unwrap();
    println!("{}", path.to_string_lossy())
}

#[test]
fn test_sdk_fmt() {
    println!("{}", Sdk::Built(BuiltinSdk::Java))
}