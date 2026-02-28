use sdkcore::env::{EnvOperation, windows::WindowsEnvOperation};

#[test]
fn test_windows_env() {
    let window = WindowsEnvOperation {};
    let sdk_path = "D:\\tmp\\link_test";
    window.set_sdk_env(util::sdk::Sdk::Java, sdk_path).unwrap();
    window.add_sdk_path(sdk_path).unwrap();
}
