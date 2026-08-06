use util::consts::BugReportError;

/// 测试 BugReportError 包装与检测机制
#[test]
fn test_bug_report_error_detection() {
    // 1. 创建原始错误
    let original_msg = "some unexpected system error";
    let err = anyhow::anyhow!(original_msg);

    // 2. 包装为 BugReportError
    let wrapped = BugReportError::wrap(err);

    // 3. downcast_ref 能检测到 BugReportError
    assert!(
        wrapped.downcast_ref::<BugReportError>().is_some(),
        "BugReportError should be detectable via downcast_ref"
    );

    // 4. Display 输出保持原始错误消息，不附加标记
    assert_eq!(
        wrapped.to_string(),
        original_msg,
        "BugReportError Display should preserve original error message"
    );

    // 5. 普通错误（无 BugReportError 包装）不应被检测到
    let normal_err = anyhow::anyhow!("user error: SDK not found");
    assert!(
        normal_err.downcast_ref::<BugReportError>().is_none(),
        "Normal error should NOT be detected as BugReportError"
    );
}

/// 测试 BugReportError 嵌套 context 链后仍可检测
#[test]
fn test_bug_report_error_with_context_chain() {
    let inner = anyhow::anyhow!("io error");
    // 先加描述性 context，再包装 BugReportError
    let wrapped = BugReportError::wrap(inner.context("Failed to create directory"));
    assert!(
        wrapped.downcast_ref::<BugReportError>().is_some(),
        "BugReportError should still be detectable after context chain"
    );
}

/// 测试 bug report URL 带 title + body 参数，预填信息方便用户直接提交 issue
#[test]
fn test_bug_report_url_contains_title_and_body() {
    let command = "switch java 21";
    let error_msg = "Failed to create symlink";

    // 构建预期的 URL 基础路径
    let base = "https://github.com/borenchan/sdkmate/issues/new";
    let url = util::terminal::build_bug_report_url(command, error_msg);
    println!("Url:{}",url);
    // URL 应以 base 开头
    assert!(url.starts_with(base), "URL should start with base path");

    // 验证 URL 合法可解析
    let parsed = url::Url::parse(&url).expect("URL should be valid");
    let params: std::collections::HashMap<String, String> =
        parsed.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();

    // URL 应包含 title 和 body 参数
    assert!(params.contains_key("title"), "URL should have title param");
    assert!(params.contains_key("body"), "URL should have body param");

    // title 格式：[issue] <error_summary> — <command>，问题重点在前
    let title = params.get("title").unwrap();
    assert!(title.starts_with("[issue]"), "title should start with [issue]");
    assert!(title.contains("symlink"), "title should highlight error first");
    assert!(title.contains("switch"), "title should contain command as context");

    // body 应包含关键信息区域
    // body 标签改为 Issue Report（而非 Bug Report）
    let body = params.get("body").unwrap();
    assert!(body.contains("Issue Report"), "body should use Issue Report header");
    assert!(body.contains("Command"), "body should contain command section");
    assert!(body.contains("Error"), "body should contain error section");
    assert!(body.contains("sdkm version"), "body should contain sdkm version section");
    assert!(body.contains("OS"), "body should contain OS section");
    assert!(body.contains("Platform"), "body should contain platform section");
    assert!(body.contains("Steps to reproduce"), "body should contain steps section");
}

/// 测试长错误消息在 title 中的截断
#[test]
fn test_bug_report_url_truncates_long_error() {
    let long_error = "A very long error message that exceeds the 80 character limit for the title and should be truncated with ellipsis at the end";
    let url = util::terminal::build_bug_report_url("install node 18", long_error);

    // 解析 URL 获取 title 参数值
    let parsed = url::Url::parse(&url).expect("URL should be valid");
    let params: std::collections::HashMap<String, String> =
        parsed.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();

    let title = params.get("title").expect("title param should exist");
    // 截断后的 title 应包含 "...", 且总长度合理
    assert!(title.contains("..."), "Long error in title should be truncated with ...");
}
