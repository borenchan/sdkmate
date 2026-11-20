use crate::report::ProgressHandle;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 解压下载的压缩包到指定目录，支持进度反馈
/// - Windows / .zip → 使用 zip crate，逐文件进度
/// - .tar.gz → 使用 flate2 + tar crate，spinner 动画（gzip 流不支持 Seek，无法手动迭代文件）
pub fn extract_archive(archive_path: &Path, dest_dir: &Path, pb: &dyn ProgressHandle) -> Result<()> {
    if pb.is_cancelled() {
        bail!("install cancelled");
    }
    let ext = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    fs::create_dir_all(dest_dir).context("Failed to create extraction directory")?;

    if ext == "zip"
        || archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".zip"))
    {
        extract_zip(archive_path, dest_dir, pb)
    } else if archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".tar.gz"))
    {
        extract_tar_gz(archive_path, dest_dir, pb)
    } else {
        bail!(
            "Unsupported archive format: {}. Only .zip and .tar.gz are supported.",
            archive_path.display()
        )
    }
}

/// 解压 .zip 文件，逐文件更新进度条
fn extract_zip(archive_path: &Path, dest_dir: &Path, pb: &dyn ProgressHandle) -> Result<()> {
    let file =
        fs::File::open(archive_path).context(format!("Failed to open zip archive: {}", archive_path.display()))?;

    let mut archive = zip::ZipArchive::new(file).context("Failed to read zip archive")?;

    // zip 支持随机访问，可以预知文件总数
    let total = archive.len() as u64;
    pb.set_length(total);

    for i in 0..archive.len() {
        if pb.is_cancelled() {
            bail!("install cancelled");
        }
        let mut entry = archive.by_index(i).context(format!("Failed to read zip entry #{}", i))?;

        let entry_path = entry.enclosed_name().context(format!("Invalid zip entry path at #{}", i))?;

        let out_path = dest_dir.join(entry_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).context(format!("Failed to create directory: {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create parent directory: {}", parent.display()))?;
            }
            let mut out_file =
                fs::File::create(&out_path).context(format!("Failed to create file: {}", out_path.display()))?;
            io::copy(&mut entry, &mut out_file)
                .context(format!("Failed to write zip entry to: {}", out_path.display()))?;
            drop(out_file);

            // 恢复 zip entry 记录的 Unix 权限位（bun/deno 等 mac/Linux zip 分发的可执行文件
            // 打包时即 755；不恢复则 fs::create 默认 644，运行时 permission denied）
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out_path, fs::Permissions::from_mode(mode & 0o7777))
                    .context(format!("Failed to restore permissions on {}", out_path.display()))?;
            }
        }

        pb.inc(1);
    }

    Ok(())
}

/// 解压 .tar.gz 文件
/// 注意：gzip 解码流不支持 Seek，无法使用 tar::Archive::entries() 手动迭代
/// 只能使用 archive.unpack() 整体解压，进度由外部 spinner 动画表示
fn extract_tar_gz(archive_path: &Path, dest_dir: &Path, pb: &dyn ProgressHandle) -> Result<()> {
    if pb.is_cancelled() {
        bail!("install cancelled");
    }
    let file =
        fs::File::open(archive_path).context(format!("Failed to open tar.gz archive: {}", archive_path.display()))?;

    let gz_decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz_decoder);

    archive.unpack(dest_dir).context("Failed to extract tar.gz archive")?;
    if pb.is_cancelled() {
        bail!("install cancelled");
    }

    Ok(())
}

/// 解压后目录调整（布局归一化）：确保版本目录直接包含 SDK 文件、bin_dir 可见。
/// 依次处理三种已知包形态：
/// 1. 单层包装目录（node/go/maven...）：顶层子目录内容提升到版本目录
/// 2. macOS JDK bundle（jdk-x/Contents/Home）：直接取 Home 内容，残壳随 Contents 整体删除
/// 3. 双层包装目录（python-build-standalone full 包的 cpython-.../python/）：再提升一次
///
/// 使用 move_dir 而非直接 fs::rename，确保跨卷/父目录不存在时也能成功
pub fn normalize_extracted_dir(extracted_dir: &Path, target_dir: &Path, bin_dir: Option<&str>) -> Result<()> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(extracted_dir)
        .context("Failed to read extracted directory")?
        .collect::<io::Result<Vec<_>>>()
        .context("Failed to list extracted directory entries")?;

    if entries.len() == 1 && entries[0].path().is_dir() {
        let single_subdir = entries[0].path();
        move_dir(&single_subdir, target_dir)?;
        let _ = fs::remove_dir(extracted_dir);
    } else {
        move_dir(extracted_dir, target_dir)?;
    }

    // macOS JDK bundle 摊平（bin_dir 锚点驱动；趁 Contents 尚未被提升时处理，
    // 须在单子目录提升之后、双层提升之前）
    flatten_macos_bundle(target_dir, bin_dir)?;

    // 双层提升：处理双层嵌套（如 cpython-...-install_only/python/）
    // 如果 target_dir 仍只有一个子目录，提升其内容到 target_dir
    lift_single_inner_dir(target_dir)?;

    Ok(())
}

/// 如果 target_dir 内只有一个子目录，将其内容提升到 target_dir 本身。
/// 处理双层嵌套的归档（如 python-build-standalone install_only）。
fn lift_single_inner_dir(target_dir: &Path) -> Result<()> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(target_dir)
        .context("Failed to read target directory for inner lift")?
        .collect::<io::Result<Vec<_>>>()
        .context("Failed to list target directory entries")?;

    if entries.len() != 1 || !entries[0].path().is_dir() {
        return Ok(()); // 不是单子目录，无需提升
    }

    let inner_dir = entries[0].path();

    // 使用临时 staging 目录避免移动冲突：inner_dir 在 target_dir 内，
    // 不能直接 rename inner_dir → target_dir（target_dir 是 inner_dir 的父目录）
    let staging_dir = target_dir.parent().unwrap_or(target_dir).join(format!(
        "{}_staging",
        target_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));

    // 将 inner_dir 移到 staging
    move_dir(&inner_dir, &staging_dir)?;

    // target_dir 现已清空，将 staging（即原 inner_dir 内容）移回 target_dir
    move_dir(&staging_dir, target_dir)?;

    Ok(())
}

/// macOS JDK bundle 摊平。
/// macOS JDK 包是 .app bundle（jdk-x/Contents/Home）：单子目录提升 jdk-x 后版本目录 = [Contents]，
/// 直接取 Contents/Home 的子项提升到版本根，bundle 元数据（MacOS/_CodeSignature/Info.plist）
/// 随 Contents 整体删除——残壳从不进入版本目录。
/// 以 bin_dir 为锚点：Contents/Home 内 bin_dir 可见才动作，其余形态零动作。
/// 不做平台 gate：Windows/Linux 包不会出现该布局，结构检测天然安全
fn flatten_macos_bundle(target_dir: &Path, bin_dir: Option<&str>) -> Result<()> {
    let Some(dir) = bin_dir else {
        return Ok(()); // 无锚点（根二进制 SDK）不动作
    };
    let contents = target_dir.join("Contents");
    let home = contents.join("Home");
    if !home.join(dir).is_dir() {
        return Ok(()); // 非 bundle 形态（含正常包 bin 已在版本根），交给后续步骤/校验
    }

    // Home 子项（bin/lib/conf...）与 Contents 不同名，逐项提升安全
    let entries: Vec<fs::DirEntry> = fs::read_dir(&home)
        .context("Failed to read macOS bundle Home directory")?
        .collect::<io::Result<Vec<_>>>()
        .context("Failed to list macOS bundle Home entries")?;
    for entry in entries {
        let src = entry.path();
        let dst = target_dir.join(entry.file_name());
        if src.is_dir() {
            move_dir(&src, &dst)?;
        } else {
            // 普通文件（如 JDK 的 release）：move_dir 的 copy 回退仅支持目录，单独处理
            if fs::rename(&src, &dst).is_ok() {
                continue;
            }
            fs::copy(&src, &dst).context(format!("Failed to copy file: {}", src.display()))?;
            fs::remove_file(&src).context(format!("Failed to remove source file after copy: {}", src.display()))?;
        }
    }

    // bundle 元数据随 Contents 整体删除
    fs::remove_dir_all(&contents).context(format!("Failed to remove macOS bundle Contents: {}", contents.display()))?;

    Ok(())
}

/// 限深递归搜索 bin_dir 实际所在目录（相对 version_dir 的路径）。
/// 供安装校验的软警告做诊断提示（bin_dir 不在版本根时给出实际位置与修正命令）
pub fn locate_bin_dir(version_dir: &Path, bin_dir: &str) -> Option<PathBuf> {
    fn search(dir: &Path, rel: &Path, bin_dir: &str, depth: usize) -> Option<PathBuf> {
        if depth == 0 {
            return None;
        }
        let entries: Vec<fs::DirEntry> = fs::read_dir(dir).ok()?.flatten().collect();
        // 本层优先：先扫同名目录再深入，bin 通常在浅层
        for entry in &entries {
            if entry.file_name() == bin_dir && entry.path().is_dir() {
                return Some(rel.join(bin_dir));
            }
        }
        for entry in &entries {
            let name = entry.file_name();
            if name != bin_dir
                && entry.path().is_dir()
                && let Some(hit) = search(&entry.path(), &rel.join(&name), bin_dir, depth - 1)
            {
                return Some(hit);
            }
        }
        None
    }
    search(version_dir, Path::new(""), bin_dir, 4)
}

/// 移动目录：优先 fs::rename（快速），失败则回退到 copy + remove（慢但可靠）
/// 处理 Windows 特有问题：
///   1. 目标父目录不存在时 rename 失败 → 先 create_dir_all
///   2. 文件被锁/跨卷 rename 失败 → 回退 copy_dir_recursive + remove_dir_all
/// 移动目录（同卷 rename，跨卷/失败回退递归拷贝；目标已存在则先删除）
pub(crate) fn move_dir(src: &Path, dst: &Path) -> Result<()> {
    // 1. 确保目标父目录存在（Windows rename 要求目标父目录必须存在）
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .context(format!("Failed to create target parent directory: {}", parent.display()))?;
    }

    // 2. 目标已存在则先删除
    if dst.exists() {
        fs::remove_dir_all(dst).context(format!("Failed to remove existing target directory: {}", dst.display()))?;
    }

    // 3. 优先 rename（同卷时是原子操作，极快）
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    // 4. rename 失败（跨卷 / Windows 文件锁），回退到 copy + remove
    copy_dir_recursive(src, dst)?;
    fs::remove_dir_all(src).context(format!("Failed to remove source after copy: {}", src.display()))?;

    Ok(())
}

/// 递归复制目录（用于 rename 失败时的回退方案）
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).context(format!("Failed to create directory: {}", dst.display()))?;

    for entry in fs::read_dir(src).context(format!("Failed to read directory: {}", src.display()))? {
        let entry = entry.context("Failed to read directory entry")?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).context(format!(
                "Failed to copy file: {} → {}",
                src_path.display(),
                dst_path.display()
            ))?;
        }
    }

    Ok(())
}

/// 验证解压结果：完整性硬校验（目录存在且非空）。
/// bin_dir 布局校验归 install 流程（软警告诊断，不回滚）
pub fn verify_extraction(target_dir: &Path) -> Result<()> {
    if !target_dir.exists() {
        bail!(
            "Extraction verification failed: directory {} does not exist",
            target_dir.display()
        )
    }

    let entries = fs::read_dir(target_dir).context("Failed to read extracted target directory")?;
    if entries.count() == 0 {
        bail!("Extraction verification failed: directory {} is empty", target_dir.display())
    }

    Ok(())
}
