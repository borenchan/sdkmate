#!/usr/bin/env python3
"""给指定 issue 检索相似 issue，输出 Markdown 评论正文（无相似项时输出为空）。

用法：similar_issues.py <issues.json> <issue_number>
- issues.json：`gh api --paginate --slurp repos/{repo}/issues?state=all&per_page=100` 的输出
- 相似度：中文按二元组、英文按单词切分，关键词重叠数 >= MIN_SCORE 视为相似
"""

import json
import re
import sys

MIN_SCORE = 3
MAX_RESULTS = 3
# 高频通用词不参与相似度（否则"版本管理""能否支持"这类套话会制造假阳性）
STOPWORDS = {
    "版本", "本管", "管理", "支持", "功能", "建议", "许愿", "考虑", "加入",
    "能否", "是否", "如何", "怎么", "可以", "无法", "不能", "正常", "工作",
    "问题", "报错", "错误", "失败", "安装", "下载", "使用", "出现", "一个",
}


def tokens(text: str) -> set[str]:
    """标题 → 关键词集合（英文/数字单词 + 中文二元组）"""
    text = text.lower()
    words = set(re.findall(r"[a-z0-9_+.-]{2,}", text))
    grams: set[str] = set()
    for seg in re.findall(r"[一-鿿]+", text):
        if len(seg) == 1:
            grams.add(seg)
        for i in range(len(seg) - 1):
            grams.add(seg[i : i + 2])
    return (words | grams) - STOPWORDS


def main() -> None:
    path, current_no = sys.argv[1], int(sys.argv[2])
    with open(path, encoding="utf-8") as f:
        issues = json.load(f)

    current = next((i for i in issues if i["number"] == current_no), None)
    if current is None:
        return

    me = tokens(current.get("title") or "")
    scored: list[tuple[int, int, str, str]] = []
    for it in issues:
        # 跳过 PR 与自身
        if it.get("pull_request") or it["number"] == current_no:
            continue
        score = len(me & tokens(it.get("title") or ""))
        if score >= MIN_SCORE:
            scored.append((score, it["number"], it.get("title") or "", it.get("state", "open")))

    scored.sort(reverse=True)
    top = scored[:MAX_RESULTS]
    if not top:
        return

    print("🤖 **自动查重提示**\n")
    print("发现以下 issue 与本问题相似，可能是重复：\n")
    for _score, num, title, state in top:
        flag = "（已关闭）" if state == "closed" else ""
        print(f"- #{num} {title} {flag}")
    print("\n如其中已有相同问题，可直接关闭本 issue 并说明；维护者会据此合并处理。")


if __name__ == "__main__":
    main()