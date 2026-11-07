//! 错误标记宏。终端输出宏已随渲染能力迁至 cli crate（util 保持 UI 无关，GUI 铺路）。

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
