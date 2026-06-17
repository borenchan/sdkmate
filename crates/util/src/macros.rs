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
