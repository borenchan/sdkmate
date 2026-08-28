# sdkm 一键安装脚本（Windows PowerShell）
# 用法：powershell -ExecutionPolicy Bypass -File install.ps1 [-Dest 安装目录]   默认安装到 %USERPROFILE%\.sdkm
# 流程：拉取最新 release 资产 → 解压 → 初始化
# 注意：init/switch 需管理员权限（写注册表 + 创建符号链接），请用管理员终端运行
param(
    [string]$Dest = "$env:USERPROFILE\.sdkm"
)

$ErrorActionPreference = "Stop"

# ── 1. 平台校验（当前 release 仅 x64 Windows 产物）──
if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Host "❌ 暂无 32 位 Windows 产物，请到 https://github.com/borenchan/sdkmate/releases 反馈" -ForegroundColor Red
    exit 1
}
$asset = "sdkm-windows-x86_64.zip"

# ── 2. 从 GitHub API 拿最新版资产下载地址 ──
$api = "https://api.github.com/repos/borenchan/sdkmate/releases/latest"
Write-Host "ℹ️  查询最新版本资产: $asset"
try {
    $release = Invoke-RestMethod -Uri $api -UseBasicParsing
} catch {
    Write-Host "❌ 无法访问 GitHub API（检查网络/代理）: $_" -ForegroundColor Red
    exit 1
}
$downloadUrl = ($release.assets | Where-Object { $_.name -eq "$asset" }).browser_download_url
if (-not $downloadUrl) {
    Write-Host "❌ 未找到资产 $asset，请到 https://github.com/borenchan/sdkmate/releases 手动下载" -ForegroundColor Red
    exit 1
}

# ── 3. 下载并解压 ──
$tmp = Join-Path $env:TEMP ("sdkm_install_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $tmp | Out-Null
$zipPath = Join-Path $tmp "sdkm.zip"
Write-Host "⬇️  下载 $downloadUrl"
Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing
Expand-Archive -Path $zipPath -DestinationPath $tmp -Force   # 解压得 .sdkm\sdkm.exe

# ── 4. 安装到目标目录（已装则覆盖二进制，保留 store/ 与配置）──
New-Item -ItemType Directory -Force $Dest | Out-Null
if (Test-Path (Join-Path $Dest "config.toml")) {
    Copy-Item (Join-Path $tmp ".sdkm\sdkm.exe") $Dest -Force
    Write-Host "✅ 已覆盖二进制到 $Dest（原有 store/ 与 config.toml 保留）"
} else {
    Copy-Item (Join-Path $tmp ".sdkm\*") $Dest -Recurse -Force
    Write-Host "✅ 已安装到 $Dest"
}
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue

# ── 5. 初始化 + PATH 提示 ──
$sdkmExe = Join-Path $Dest "sdkm.exe"
& $sdkmExe init
if ($LASTEXITCODE -ne 0) {
    Write-Host "⚠️  init 未完成（可稍后手动执行），安装目录: $Dest" -ForegroundColor Yellow
}
# init 已把安装目录注册进系统 PATH（注册表）；当前会话即时生效需手动刷
if (-not (($env:Path -split ';') -contains $Dest)) {
    $env:Path = "$Dest;$env:Path"
}
Write-Host "🎉 安装完成: $(& $sdkmExe --version)"
Write-Host "👉 若新终端里 sdkm 不生效，请重开终端（注册表 PATH 已写入）"
