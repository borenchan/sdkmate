#!/usr/bin/env bash
set -euo pipefail
# 由 workflow 注入 VERSION（如 0.2.0）
: "${VERSION:?}"
REPO_FULL="borenchan/sdkmate"
CUR_TAG="v${VERSION}"

# 找上一个 tag（按版本号倒序，取当前 tag 的下一行）
TAGS=$(git tag --sort=-v:refname --list 'v[0-9]*')
PREV_TAG=$(printf '%s\n' "$TAGS" | grep -A1 -xF "$CUR_TAG" | tail -1 || true)
if [ -z "$PREV_TAG" ] || [ "$PREV_TAG" = "$CUR_TAG" ]; then
  RANGE=""; RANGE_DESC=""
else
  RANGE="${PREV_TAG}..HEAD"; RANGE_DESC="${PREV_TAG}...${CUR_TAG}"
fi

# 拿到 短SHA + subject（每行一个）
# --invert-grep --grep：排除 CI 回写 CHANGELOG.md 的提交（docs(changelog): update for vX.Y.Z），
# 否则每次发版的 changelog 都会带上一次发版回写 CHANGELOG.md 的噪音提交，形成循环
mapfile -t COMMITS < <(git log ${RANGE:+"$RANGE"} --pretty=format:"%h %s" --invert-grep --grep="^docs(changelog):" 2>/dev/null || true)
if [ "${#COMMITS[@]}" -eq 0 ]; then
  echo "（无提交记录）"; exit 0
fi

# emit <type> <emoji> <title>：输出该类型的所有提交
emit() {
  local type="$1" emoji="$2" title="$3" first=1 line sha subject desc
  # 正则存变量，避免 shell 解析括号；匹配 type(可选scope): 描述
  local re="^${type}(\([^)]+\))?:[[:space:]]*(.+)$"
  for line in "${COMMITS[@]}"; do
    sha="${line%% *}"
    subject="${line#* }"
    if [[ "$subject" =~ $re ]]; then
      desc="${BASH_REMATCH[2]}"
      if [ $first -eq 1 ]; then printf '\n### %s %s\n\n' "$emoji" "$title"; first=0; fi
      printf -- '- %s — [`%s`](https://github.com/%s/commit/%s)\n' "$desc" "$sha" "$REPO_FULL" "$sha"
    fi
  done
}

printf '## 🚀 What Changed\n'
emit feat     "✨" "Features"
emit fix      "🐛" "Bug Fixes"
emit perf     "⚡" "Performance"
emit refactor "♻️" "Refactor"
emit docs     "📝" "Documentation"
emit test     "✅" "Tests"
emit style    "🎨" "Style"
emit ci       "👷" "CI / Build"
emit build    "📦" "Build"
emit chore    "🧰" "Chore"

# 未归类（非约定式提交）
other_re="^(feat|fix|perf|refactor|docs|test|style|ci|build|chore)(\([^)]+\))?:"
first=1
for line in "${COMMITS[@]}"; do
  sha="${line%% *}"; subject="${line#* }"
  if [[ ! "$subject" =~ $other_re ]]; then
    if [ $first -eq 1 ]; then printf '\n### 📌 Other\n\n'; first=0; fi
    printf -- '- %s — [`%s`](https://github.com/%s/commit/%s)\n' "$subject" "$sha" "$REPO_FULL" "$sha"
  fi
done

if [ -n "$RANGE_DESC" ]; then
  printf '\n**Full Changelog**: https://github.com/%s/compare/%s\n' "$REPO_FULL" "$RANGE_DESC"
fi
