> **禁止纯 AI 生成**：提交者必须逐行 review 自己的全部改动并理解其行为，未经人工 review 的 AI 生成内容不予接收。

## 改了什么

<!-- 一两句话说清改动内容 -->

## 为什么改

<!-- 动机、关联 issue：(closes #N) -->

## 怎么验证的

<!-- 命令与结果，如 `cargo test -p sdkm-core` 全绿，或行为截图 -->

## 自查

- [ ] 一个 PR 只做一件事，没有夹带无关修改
- [ ] 已 rebase 到最新 master，本地 `cargo test` 通过
- [ ] 涉及 `#[cfg(unix)]` 的改动已在 Windows + Linux 双平台编译验证
