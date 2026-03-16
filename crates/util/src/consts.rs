pub const BANNER: &'static str = r#"
                                 ,--.           ____
  .--.--.        ,---,        ,--/  /|         ,'  , `.
 /  /    '.    .'  .' `\   ,---,': / '      ,-+-,.' _ |
|  :  /`. /  ,---.'     \  :   : '/ /    ,-+-. ;   , ||
;  |  |--`   |   |  .`\  | |   '   ,    ,--.'|'   |  ;|
|  :  ;_     :   : |  '  | '   |  /    |   |  ,', |  ':
 \  \    `.  |   ' '  ;  : |   ;  ;    |   | /  | |  ||
  `----.   \ '   | ;  .  | :   '   \   '   | :  | :  |,
  __ \  \  | |   | :  |  ' |   |    '  ;   . |  ; |--'
 /  /`--'  / '   : | /  ;  '   : |.  \ |   : |  | ,
'--'.     /  |   | '` ,/   |   | '_\.' |   : '  |/
  `--'---'   ;   :  .'     '   : |     ;   | |`-'
             |   ,.'       ;   |,'     |   ;/
             '---'         '---'       '---'
"#;

pub const UNKNOWN: &'static str = "unknown";

pub const SIZE_KB: u64 = 1024;
pub const SIZE_MB: u64 = 1024 * 1024;
pub const SIZE_GB: u64 = SIZE_MB * 1024;

pub const ENV_PATH: &'static str = "PATH";
pub const ENV_JAVA_HOME: &'static str = "JAVA_HOME1";

pub const SDKM_ROOT_DIR: &'static str = "store";


// default symlink dir
#[cfg(windows)]
pub const SDKM_SYMLINK_DIR: &'static str = "C:\\Program Files\\sdkm";

#[cfg(unix)]
pub const SDKM_SYMLINK_DIR: &'static str = "/usr/local/sdkm";