use sdkcore::link::symlink;
use std::fs;

/// 生成独立临时目录，避免并行测试冲突
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sdkm_test_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_create_and_remove_symlink() {
    let temp = temp_dir("basic");
    let original = temp.join("original");
    fs::create_dir_all(&original).unwrap();

    let link = temp.join("link_test");

    // Create the symlink
    symlink::create_symlink(&original, &link).unwrap();
    assert!(link.exists());
    assert!(link.is_symlink());

    // Remove the symlink
    symlink::remove_symlink(&link).unwrap();
    assert!(!link.exists());

    // Cleanup temp dir
    let _ = fs::remove_dir_all(&temp);
}

/// switch 场景：已有有效 symlink，create_symlink 应替换为新目标
#[test]
fn test_create_symlink_replaces_existing() {
    let temp = temp_dir("replace");
    let original1 = temp.join("original1");
    let original2 = temp.join("original2");
    fs::create_dir_all(&original1).unwrap();
    fs::create_dir_all(&original2).unwrap();

    let link = temp.join("link_replace");

    // 先创建指向 original1 的 symlink
    symlink::create_symlink(&original1, &link).unwrap();
    assert!(link.exists());

    // 替换为指向 original2 的 symlink
    symlink::create_symlink(&original2, &link).unwrap();

    // 验证 link 现在指向 original2
    let target = fs::read_link(&link).unwrap();
    assert!(target.ends_with("original2"), "symlink should point to original2");

    let _ = fs::remove_dir_all(&temp);
}

/// issue #6 核心场景：断链 symlink（目标已删）存在时，create_symlink 应删除并重建
#[test]
fn test_create_symlink_replaces_dangling() {
    let temp = temp_dir("dangling");
    let original = temp.join("original");
    let will_delete = temp.join("will_delete");
    fs::create_dir_all(&original).unwrap();
    fs::create_dir_all(&will_delete).unwrap();

    let link = temp.join("link_dangling");

    // 先创建指向 will_delete 的 symlink
    symlink::create_symlink(&will_delete, &link).unwrap();

    // 删除目标目录，制造断链
    fs::remove_dir_all(&will_delete).unwrap();
    // 断链时 exists() 返回 false（跟随 symlink 到不存在的目标）
    assert!(!link.exists(), "dangling symlink should not 'exist'");

    // 断链状态下创建新 symlink——原来这里会报 os error 183
    symlink::create_symlink(&original, &link).unwrap();

    // 验证新 symlink 指向 original
    let target = fs::read_link(&link).unwrap();
    assert!(target.ends_with("original"), "symlink should point to original");
    assert!(link.exists(), "new symlink should be valid");

    let _ = fs::remove_dir_all(&temp);
}

/// remove_symlink 应能删除断链 symlink（修复前断链时 exists()=false 会跳过）
#[test]
fn test_remove_symlink_dangling() {
    let temp = temp_dir("rm_dangling");
    let will_delete = temp.join("will_delete");
    fs::create_dir_all(&will_delete).unwrap();

    let link = temp.join("link_rm_dangling");

    // 创建 symlink 后删除目标，制造断链
    symlink::create_symlink(&will_delete, &link).unwrap();
    fs::remove_dir_all(&will_delete).unwrap();
    assert!(!link.exists());

    // remove_symlink 应实际删除断链 symlink（而非跳过）
    symlink::remove_symlink(&link).unwrap();

    // symlink_metadata 也应失败——symlink 确实被删了
    assert!(
        fs::symlink_metadata(&link).is_err(),
        "dangling symlink should be removed"
    );

    let _ = fs::remove_dir_all(&temp);
}

/// remove_symlink 对不存在的路径返回 Ok（无需删除）
#[test]
fn test_remove_symlink_nonexistent() {
    let temp = temp_dir("rm_nonexist");
    let link = temp.join("link_nonexistent");

    // 路径不存在时返回 Ok
    symlink::remove_symlink(&link).unwrap();

    let _ = fs::remove_dir_all(&temp);
}
