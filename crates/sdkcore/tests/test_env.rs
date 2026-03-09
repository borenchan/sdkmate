use sdkcore::env::{EnvOperation, OsOperation};

#[test]
fn test_windows_env() {
    let window = OsOperation {};
    let sdk_path = "D:\\tmp\\link_test";
    let string = window.get_path().unwrap();
    println!("Path:{}", string);
    window.add_sdk_path(sdk_path).unwrap();
}
