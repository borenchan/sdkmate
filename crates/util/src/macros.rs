#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::info(&msg)
    };
}
#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::success(&msg)
    };
}
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::warning(&msg)
    };
}
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::error(&msg)
    };
}

/// 辅助信息宏：URL/路径/清理等次要输出（暗灰缩进）
#[macro_export]
macro_rules! detail {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::detail(&msg)
    };
}

/// 多步骤阶段标记宏：label 为步骤名，后续为描述
#[macro_export]
macro_rules! step {
    ($label:expr, $($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::step($label, &msg)
    };
}

/// 分隔线宏
#[macro_export]
macro_rules! divider {
    () => {
        $crate::terminal::divider()
    };
}

/// 目录树宏：展示目录结构，比 detail 亮（grey 色）
#[macro_export]
macro_rules! tree {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::tree(&msg)
    };
}

/// banner 宏：无缩进输出，用于 ASCII art
#[macro_export]
macro_rules! banner {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::banner(&msg)
    };
}

/// 执行操作，失败时自动添加 BugReportError 标记并传播错误
/// 用于标记不可由用户自行解决的系统/IO意外错误
/// 用法: try_bug!(io_operation) 等同于 match io_operation { Err(e) => return Err(BugReportError::wrap(e.into())), Ok(v) => v }
#[macro_export]
macro_rules! try_bug {
    ($expr:expr) => {
        match $expr {
            Err(e) => return Err($crate::consts::BugReportError::wrap(e.into())),
            Ok(val) => val,
        }
    };
}

/// 生成不可由用户自行解决的错误并标记 BugReportError
/// 用法: bail_bug!("message") 等同于 return Err(BugReportError::wrap(anyhow::anyhow!("message")))
#[macro_export]
macro_rules! bail_bug {
    ($($arg:tt)*) => {
        return Err($crate::consts::BugReportError::wrap(anyhow::anyhow!($($arg)*)))
    };
}
