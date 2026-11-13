#!/usr/bin/env python3
"""英文 changelog → 简体中文版（release note 双语展示用）。

翻译走谷歌翻译免费网页端点（client=gtx，无需 key）；gen_changelog.sh 的分组
标题走内置映射表（不进机翻，杜绝翻错），仅逐条翻译 commit 描述文本，反引号
代码与专有名词用占位符保护。任何失败（限流/超时/解析异常）都只打警告并退出 0，
不产出输出文件，发布流程由调用端兜底为纯英文。
用法: translate_changelog_zh.py <in.md> <out.md>
"""
import json
import os
import re
import sys
import time
import urllib.parse
import urllib.request

IN_PATH, OUT_PATH = sys.argv[1], sys.argv[2]
# 端点可覆盖（本地 mock 测试用）
BASE_URL = os.environ.get("GT_BASE_URL", "https://translate.googleapis.com")

# gen_changelog.sh 输出的固定标题映射（## What Changed + 10 个分组 + Other），不进机翻
HEADINGS = {
    "What Changed": "更新内容",
    "Features": "新功能",
    "Bug Fixes": "问题修复",
    "Performance": "性能优化",
    "Refactor": "重构",
    "Documentation": "文档",
    "Tests": "测试",
    "Style": "代码风格",
    "CI / Build": "CI / 构建",
    "Build": "构建",
    "Chore": "杂项",
    "Other": "其他",
}

# 条目行外壳：- {desc} — [`sha`](commit 链接)，只把 desc 送翻译
ITEM_RE = re.compile(r"^- (.+) — \[`([0-9a-f]{7,})`\]\((https://[^)]+)\)$")
# desc 内需要保护的片段：反引号代码 + 专有名词（防被机翻改写）
PROTECT_RE = re.compile(
    r"`[^`]+`|\b(sdkm|sdkmate|sdkm-gui|GPUI|GitHub|Gitee|WSL|PATH|TUI|GUI|CLI|TOML|Java|Node\.js|Python|Maven|Go|Rust|crates?)\b"
)
PLACEHOLDER = "⟦{}⟧"  # ⟦n⟧ 生僻定界符，机翻通常原样保留
# 定界符间容忍机翻插入的空白等杂字符（用 [^0-9] 避免转义序列兼容问题）
PLACEHOLDER_RE = re.compile("⟦[^0-9]*([0-9]+)[^0-9]*⟧")
BATCH_SIZE = 30  # 单次请求条数，避免请求体过长


def translate_batch(texts):
    """谷歌免费端点批量翻译（translate_a/t 支持多 q，按序返回）。失败抛异常。"""
    data = urllib.parse.urlencode([("q", t) for t in texts]).encode()
    req = urllib.request.Request(
        BASE_URL + "/translate_a/t?client=gtx&sl=en&tl=zh-CN",
        data=data,
        headers={"User-Agent": "Mozilla/5.0"},
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        result = json.loads(resp.read().decode("utf-8"))
    # 该端点返回形态随版本在 ["译文"] 与 [["译文","en"]] 间漂移，两种都兼容
    out = [item[0] if isinstance(item, list) else item for item in result]
    if len(out) != len(texts):
        raise ValueError("translate result count mismatch: {} != {}".format(len(out), len(texts)))
    return out


def protect(text, pool):
    """把代码片段/专名换成 ⟦n⟧ 占位符，翻译后按池还原。"""
    def _sub(match):
        pool.append(match.group(0))
        return PLACEHOLDER.format(len(pool) - 1)
    return PROTECT_RE.sub(_sub, text)


def restore(text, pool):
    return PLACEHOLDER_RE.sub(lambda m: pool[int(m.group(1))], text)


def translate_heading(line):
    """分组标题查表换中文；查不到的标题原样保留。"""
    stripped = line.rstrip()
    for en, zh in HEADINGS.items():
        if stripped.endswith(" " + en):
            return stripped[: -len(en)] + zh
    return line


def main():
    with open(IN_PATH, encoding="utf-8") as f:
        lines = f.read().splitlines()

    # 单遍扫描：每行归为 标题/条目/普通行 三类；条目行抽出（占位描述, sha, 链接, 占位池）
    # Contributors（### 🙌 起）与末尾 Full Changelog 链接不翻，避免与英文区重复
    rendered, items = [], []
    for line in lines:
        if line.startswith("### \U0001f64c") or line.startswith("**Full Changelog**"):
            break
        m = ITEM_RE.match(line)
        if m:
            pool = []
            items.append((protect(m.group(1), pool), m.group(2), m.group(3), pool))
            rendered.append(("item", len(items) - 1))
        elif line.startswith("#"):
            rendered.append(("heading", line))
        else:
            rendered.append(("raw", line))

    if not items:
        print("::notice::nothing to translate, skip Chinese changelog")
        return

    descs = [it[0] for it in items]
    translated = []
    for i in range(0, len(descs), BATCH_SIZE):
        translated.extend(translate_batch(descs[i:i + BATCH_SIZE]))
        if i + BATCH_SIZE < len(descs):
            time.sleep(1)  # 批间隔歇一下，降低限流概率

    with open(OUT_PATH, "w", encoding="utf-8", newline="\n") as f:
        for kind, payload in rendered:
            if kind == "heading":
                f.write(translate_heading(payload) + "\n")
            elif kind == "item":
                _, sha, url, pool = items[payload]
                f.write("- {} — [`{}`]({})\n".format(restore(translated[payload], pool), sha, url))
            else:
                f.write(payload + "\n")
    print("Chinese changelog written: " + OUT_PATH)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # 兜底：任何失败都不阻塞发版
        print("::warning::Chinese changelog skipped: {}".format(exc))
        sys.exit(0)
