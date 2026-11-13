//! Windows 可执行文件图标：gpui 的窗口类图标从 exe 资源 ID 1 读取（不是 WindowOptions.icon），
//! 这里把品牌 ICO 嵌入 exe；非 Windows 平台不做任何事。

fn main() {
    #[cfg(windows)]
    {
        let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/gui-icons/sdkm.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon.to_string_lossy().as_ref());
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed exe icon: {e}");
        }
    }
    println!("cargo:rerun-if-changed=../../assets/gui-icons/sdkm.ico");
}
