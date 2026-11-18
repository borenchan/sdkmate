# 贡献指南

感谢你愿意为 sdkm 贡献代码！sdkm 由维护者与社区共同打磨，这份文档说明协作流程和工程约束。提交 PR 前请通读一遍。

## 设计哲学

- **完全透明**：所有行为可观察、可解释，不做暗箱操作
- **简约**：最少的代码解决明确的问题，不预埋未要求的「灵活性」和「可配置性」
- **高性能**：冷启动快、二进制小（release 开启体积优化）
- **可扩展**：数据驱动（`SDK_SEEDS` 注册表）+ 自定义 SDK 兜底

## 开始之前

1. **先搜索**：在 Issues 和 Discussions 里确认没有相同的问题或建议，避免重复劳动。
2. **先讨论**：较大的功能（新 SDK 接入、架构调整、GUI 新页面、命令行为变更）请先在 Discussions / Ideas 或相关 issue 里讨论方案，达成一致后再动手。方向不符的实现 PR 会被关闭，别浪费你的时间。
3. **认领**：在 issue 下评论 `/claim`（或 `/认领`），机器人会自动把你加入 Assignees 并打上「已认领」标签。已被他人认领的 issue 请先与认领人沟通，由维护者调整 Assignees。完成时在 PR 描述里写 `Closes #<issue号>`。
4. 不确定的地方在 issue 里提问，不要默默选一种解释硬写。

## 开发流程

1. Fork 仓库并 clone 到本地，基于**最新 master** 拉分支（`feature/xxx`、`fix/xxx`、`docs/xxx`）：
   ```bash
   git checkout -b feature/xxx
   ```
2. 开发，遵守下方「工程约束」。
3. 本地质量门禁全部通过（与 CI 一致）：
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features
   cargo test --workspace -j 2
   ```
4. **提交 PR 前先同步上游，提前解决冲突**：
   ```bash
   git remote add upstream https://github.com/borenchan/sdkmate.git   # 首次需要
   git fetch upstream
   git rebase upstream/master
   ```
   rebase 解决冲突后**重新跑一遍质量门禁**，再 `git push --force-with-lease`。带着冲突或过时基线的 PR 会在 review 时被打回重做。
5. 推送分支并开 PR，CI 自动运行。fork 首次提交的 workflow 需要维护者手动批准一次，之后自动跑——这是 GitHub 对公共仓库的默认安全策略，不是你写错了。

## 提交规范

Conventional Commits，**英文、单行简短主题**。项目按模块拆分，代码类改动在 `type` 后带二级 scope，文档/CI 类省略 scope：

```text
feat(cli): add switch command
fix(core): resolve duplicate entry on upgrade
docs: clarify claim workflow
```

- **类型**（改动性质）：`feat` / `fix` / `perf` / `refactor` / `docs` / `test` / `style` / `build` / `ci` / `chore`
- **scope**（改动所在的代码模块，可选）：`cli`（`crates/sdkm-cli`）/ `core`（`crates/sdkm-core`）/ `util`（`crates/sdkm-util`）/ `gui`（`crates/sdkm-gui`）
- **type 与 scope 语义不同、不要同名**：type 说「什么性质」，scope 说「哪个模块」，`ci(ci)` 这类重复写法没有意义
- **scope 何时省略**：`docs` / `ci` / `test` / `style` / `build` 类提交，type 已表明区域，不再带 scope（`ci: ...`、`docs: ...`）；发版用 `chore: release vX.Y.Z`，依赖升级用 `build(deps): bump ...`（与 Vite / Vue / Angular 等主流项目一致）
- **只写一个 scope**（或省略），禁止逗号分隔多 scope（`feat(cli,core): ...` 这类写法不允许）。scope 表示「行为变化发生在哪个模块」：改 core 逻辑、GUI/CLI 只是跟随适配时写 `feat(core): ...`；确实跨模块、没有单一主模块（仓库级重构、构建/依赖调整）时省略 scope
- 不要添加 AI 或工具署名（如 `Co-Authored-By: Claude`）
- 一个 PR 聚焦一件事：不要夹带无关修改（顺带改格式、别的模块、文档重排等）
- 不要提交本地生成的临时文件（测试沙箱、日志、本地配置等）

## PR 要求

- 标题用 Conventional Commits 格式
- **描述用中文、自己的话写**，讲清三点：改了什么、为什么改、怎么验证的（命令与结果）。不要贴大段 AI 生成的长文，维护者会逐条读
- 关联 issue：`Closes #N`
- 行为相关的改动附上验证方式（命令输出、截图、测试名）
- CI 必须全绿，维护者 review 通过才能合并

## 工程约束

以下约束来自项目既有架构决策，违反会导致 review 打回：

1. **分层铁律**：`sdkm-core` 零打印、零 stdin、零终端渲染依赖——过程消息走 `sdkm_core::report` 事件契约（`CoreEvent` / `EventSink`），结果走结构化返回值与 `Result::Err`；颜色、光标、prompt、表格只属于 `sdkm-cli`；`sdkm-util` 不含终端渲染。新增功能按 core（数据）/ cli（展示）/ gui（展示）三层落位。
   - runtime 由前端持有，core 只出 `*_async` API；长阻塞段统一走 `sdkm_core::runtime::run_blocking`
   - 确认交互归调用端（core 不读 stdin）
2. **错误处理**：全项目统一 `anyhow`，不用 thiserror；`BugReport` 标记只用于用户无法自行解决的错误（解压/校验失败等真 bug），权限、环境问题用普通错误
3. **注释用中文**，只简明写「这一步在干嘛」，不写长篇背景说明
4. **路径写法**：使用处禁止跨模块全路径，一律顶部 `use` 短名（写体内不写 `sdkm_util::builtin::find_seed(...)`，改为 `use sdkm_util::builtin::find_seed;`）；标准库不写 `std::fs::` 全路径
5. **精准修改**：只碰必须碰的代码，不顺手重构、不改相邻格式、不删预先存在的死代码；自己改动产生的孤儿 import / 变量要清理
6. **双平台编译**：`#[cfg(unix)]` 的代码在 Windows 上不参与编译，改 unix 代码必须强制两端验证（临时剥掉 cfg 属性跑 `cargo check`，或 WSL 构建）；涉及跨平台代码提交前保证 Windows + Linux 都能编译通过
7. **测试**：
   - 新功能需要提交测试锁定行为；bug 回归的临时验证脚本跑通即删，不提交
   - 测试绝不写真实环境（注册表、PATH、shell profile）：用 `SDKM_HOME` 指向临时目录构造沙箱
8. **依赖与发版边界**：
   - 新增依赖先讨论（二进制体积敏感，release 开启 `opt-level=s` 与 strip）
   - 不要修改根 `Cargo.toml` 的版本号（发版由维护者控制，改版本号会触发发版流程）
   - 不要修改 `.github/workflows/`（CI 与发版链由维护者维护）
9. **文档克制**：sdkm 内置 SDK 是 40+ 的统一注册表（只是下载逻辑不同），新增普通内置条目不需要在 README / 文档里单独开段落——除非有特别注意事项（本地管理、特殊布局、外部依赖）。文档从读者视角写，不从「本次改了什么」视角写
10. **`CLAUDE.md` 允许贡献者修改，但仅限「agent 需要关注的原则与注意事项」**：它是维护者与 AI 协作的内部说明，架构、命令表、发布流程等维护者章节请勿改动；每处修改需在 PR 描述中说明原因，便于维护者核对

## Review 与合并

- 维护者会逐条 review，可能要求拆分或调整
- 合并方式由维护者选择；合并后 commit 进入 master，发版时自动进入 changelog
- 对 review 意见有异议时直接在 PR 里讨论，对事不对人

再次感谢你的贡献 🎉
