use sdkcore::link::symlink;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_create_symlink() {
        let original = r"D:\develop\project\rust-project\sdkmate\target\release";
        let link = "D:\\tmp\\link_test";
        // Create the symlink
        // symlink::create_symlink(&original, &link).unwrap();
        symlink::create_symlink(&Path::new(original), &Path::new(link)).unwrap();

        // Check that the symlink was created
        assert!(Path::new(link).exists());
    }
}
