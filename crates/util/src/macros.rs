#[macro_export]
macro_rules! info {
    // 支持格式化参数
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::info(&msg)
    };
}
#[macro_export]
macro_rules! success {
    // 支持格式化参数
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::success(&msg)
    };
}
#[macro_export]
macro_rules! warning {
    // 支持格式化参数
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::warning(&msg)
    };
}
#[macro_export]
macro_rules! error {
    // 支持格式化参数
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        $crate::terminal::error(&msg)
    };
}
