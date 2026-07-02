use sdkcore::link::symlink;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_remove_symlink() {
        let temp = std::env::temp_dir().join(format!("sdkm_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp).unwrap();

        let original = temp.join("original");
        std::fs::create_dir_all(&original).unwrap();

        let link = temp.join("link_test");

        // Create the symlink
        symlink::create_symlink(&original, &link).unwrap();
        assert!(link.exists());
        assert!(link.is_symlink());

        // Remove the symlink
        symlink::remove_symlink(&link).unwrap();
        assert!(!link.exists());

        // Cleanup temp dir
        let _ = std::fs::remove_dir_all(&temp);
    }
}
