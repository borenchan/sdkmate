#!/usr/bin/env bash
# sdkm 一键安装脚本（Linux / macOS）
# 用法：bash install.sh [-d 安装目录]   默认安装到 ~/.sdkm
# 流程：探测平台 → 拉取最新 release 资产 → 解压 → 初始化
set -euo pipefail

DEST="$HOME/.sdkm"
while getopts "d:h" opt; do
  case "$opt" in
    d) DEST="$OPTARG" ;;
    h) echo "用法: bash install.sh [-d 安装目录]（默认 ~/.sdkm）"; exit 0 ;;
    *) echo "未知参数，用法: bash install.sh [-d 安装目录]"; exit 1 ;;
  esac
done

# ── 1. 探测平台，映射 release 资产名 ──
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) platform="linux" ;;
  Darwin) platform="macos" ;;
  *) echo "❌ 不支持的系统: $os（Windows 请用 install.ps1）"; exit 1 ;;
esac
case "$arch" in
  x86_64|amd64) arch_tag="x86_64" ;;
  aarch64|arm64) arch_tag="aarch64" ;;
  *) echo "❌ 不支持的架构: $arch"; exit 1 ;;
esac

# Linux 按能力选 gnu / musl：glibc >= 2.35 用 gnu，否则用全静态 musl（任意发行版可跑）
asset="sdkm-${platform}-${arch_tag}"
if [ "$platform" = "linux" ]; then
  glibc_ver="$(ldd --version 2>/dev/null | head -n1 | grep -oE '[0-9]+\.[0-9]+$' || echo 0)"
  if [ "$(printf '%s\n2.35\n' "$glibc_ver" | sort -V | head -n1)" = "2.35" ]; then
    asset="${asset}-gnu"
  else
    asset="${asset}-musl"
  fi
fi

# ── 2. 从 GitHub API 拿最新版资产下载地址 ──
api="https://api.github.com/repos/borenchan/sdkmate/releases/latest"
echo "ℹ️  查询最新版本资产: ${asset}"
url=""
for cmd in curl wget; do
  if command -v "$cmd" >/dev/null 2>&1; then
    if [ "$cmd" = "curl" ]; then
      url="$(curl -fsSL "$api" | grep -oE "\"browser_download_url\":\s*\"[^\"]*${asset}\.tar\.gz\"" | head -n1 | cut -d'"' -f4)"
    else
      url="$(wget -qO- "$api" | grep -oE "\"browser_download_url\":\s*\"[^\"]*${asset}\.tar\.gz\"" | head -n1 | cut -d'"' -f4)"
    fi
    [ -n "$url" ] && break
  fi
done
[ -z "$url" ] && { echo "❌ 未找到资产 ${asset}.tar.gz，请检查网络或到 https://github.com/borenchan/sdkmate/releases 手动下载"; exit 1; }

# ── 3. 下载并解压 ──
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
echo "⬇️  下载 ${url}"
if command -v curl >/dev/null 2>&1; then
  curl -fSL --progress-bar -o "$tmp/pkg.tar.gz" "$url"
else
  wget -q --show-progress -O "$tmp/pkg.tar.gz" "$url"
fi
tar -xzf "$tmp/pkg.tar.gz" -C "$tmp"   # 解压得 .sdkm/sdkm

# ── 4. 安装到目标目录（已装则覆盖二进制，保留 store/ 与配置）──
mkdir -p "$DEST"
if [ -f "$DEST/config.toml" ]; then
  cp "$tmp/.sdkm/sdkm" "$DEST/sdkm"
  echo "✅ 已覆盖二进制到 ${DEST}（原有 store/ 与 config.toml 保留）"
else
  cp -r "$tmp/.sdkm/." "$DEST/"
  echo "✅ 已安装到 ${DEST}"
fi

# ── 5. 初始化 + PATH 提示 ──
"$DEST/sdkm" init || echo "⚠️  init 未完成（可稍后手动执行），安装目录: ${DEST}"
case ":$PATH:" in
  *":$DEST:"*) ;; # 已在 PATH
  *) echo "👉 请将安装目录加入 PATH 并重载 shell："
     echo "   echo 'export PATH=\"$DEST:\$PATH\"' >> ~/.$(basename "$(printf '%s' "${SHELL:-/bin/bash}" | tail -c +2)")rc && source ~/.$(basename "$(printf '%s' "${SHELL:-/bin/bash}" | tail -c +2)")rc" ;;
esac
echo "🎉 安装完成: $("$DEST/sdkm" --version 2>/dev/null || echo sdkm)"
