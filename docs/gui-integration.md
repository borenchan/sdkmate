# GUI 集成指南

> sdkmate 的 CLI/GUI 分层已就绪：`sdkcore` 是纯数据 API（零打印、零 stdin、零终端依赖），
> `cli` 是终端前端。GUI 作为另一前端接入 core，本文描述接入约定。

## 分层铁律

```
┌─ sdkcore（纯业务 + 事件契约）────────────────────┐
│  SdkManager { config, env_operation, sink }      │
│  report: CoreEvent / EventSink / ProgressKind    │
│  零打印、零 stdin、零 crossterm/indicatif 依赖   │
└──────────────────────────────────────────────────┘
        ↑                    ↑
┌─ cli（终端前端）┐   ┌─ gui（未来前端）┐
│ TerminalSink     │   │ GuiSink（channel │
│ 命令解析/确认/渲染│  │ 转发到 UI 线程） │
└──────────────────┘   └──────────────────┘
```

- **文案归 core**（事件说什么），**样式归前端**（怎么渲染：颜色/图标/进度条模板）
- 命令"结果"走返回值（如 `local_overview_rows`、`collect_doctor_report`、`InitSummary`），
  过程消息走事件流；最终失败统一走 `Result::Err`——core 在返回 Err 时**不会**对同一失败再发
  `Error` 事件，调用端只渲染一处
- `cargo tree -p sdkcore -p util` 已零终端渲染库（crossterm/unicode-width/indicatif 仅 cli 直接依赖）

## 事件契约（sdkcore::report）

- `CoreEvent`：Info / Success / Warning / Error / Detail / Step{label, msg}
- `EventSink: Send + Sync`：`emit(CoreEvent)` + `progress(ProgressKind, title) -> Box<dyn ProgressHandle>`
  （便捷方法 `info()/success()/...` 默认转发 emit）
- `ProgressKind`：7 场景枚举（Download / SelfUpdateDownload / ExtractZip / ExtractTarGz /
  Resolve / Verify / SelfUpdateExtract），前端按场景选模板/控件；core 不制造虚假百分比
  （流式场景报 spinner 型）
- `ProgressHandle`：set_length / set_position / inc / set_message / finish_with_message /
  finish_and_clear；**Drop 兜底**——core 错误路径 `?` 提前返回时，实现应在 Drop 清理未 finish
  的进度展示

### GuiSink 参考形态

```rust
struct GuiSink { tx: Sender<CoreEvent> }
impl EventSink for GuiSink {
    fn emit(&self, ev: CoreEvent) { let _ = self.tx.send(ev); }
    fn progress(&self, kind: ProgressKind, title: String) -> Box<dyn ProgressHandle> {
        // 进度事件同样经 tx 转发，或映射到 UI 线程的进度控件句柄
        Box::new(GuiProgress::new(self.tx.clone(), kind, title))
    }
}
```

## 调用约定（硬约束）

1. **每个后台操作独立构造 Manager + Sink**：`SdkManager::new(Arc::new(GuiSink::new(tx)))`。
   sink 随 manager 注入，不跨操作共享；**第一版操作串行执行**——不允许多个并发操作共享
   同一无法区分任务的消息通道（未来需要并发时再引入 operation_id 扩展）
2. **后台专用线程调用**：core 公开 API 全同步、内部 `block_on`——**勿在 async 上下文直接
   调用**（嵌套 runtime 会 panic）。约定：后台线程内构造并使用 SdkManager，只跨线程传事件/结果，
   不跨线程传 manager
3. **不引入全局 EventBus / 任务调度中心**；事件只管过程，最终成败走 `Result`

## 确认交互（core 零 stdin）

破坏性/模糊操作的确认在调用端完成：

- `resolve_install_version(sdk, input) -> ResolvedVersion`（含 `fuzzy_matched` 标记）→
  GUI 弹窗确认 → `install_sdk(sdk, &resolved, auto_switch)`
- `resolve_installed_version(sdk, input) -> ResolvedLocalVersion`（switch 确认用）
- `resolve_uninstall_target(sdk, input) -> ResolvedUninstall{version, fuzzy_matched, is_active,
  other_version}` → GUI 弹窗展示风险 → `uninstall_sdk(sdk, version_input)`
- init：`sdkcore::init::check_init_env(force) -> InitCheck`（目录警告/覆盖确认数据）→ GUI 确认 →
  `SdkManager::init_sdkm(sink, force) -> InitSummary`（目录树数据，前端渲染）

**安全边界**：resolve 类接口只用于展示与决策，**不能替代执行段自查**——用户确认期间
SDK/路径/激活状态可能变化，install/switch/uninstall 执行段仍会重算关键状态。

## 系统能力接口（EnvOperation）

`EnvOperation`（注册表/profile 读写）**不感知 UI**：写方法返回 `Vec<EnvAction>`（动作清单：
EnvSet/EnvRestored/EnvMissing/EnvRemoved/PathExists/PathAdded/PathRemoved），
业务层经 `env::report_env_actions(sink, &actions)` 转成事件。GUI 若自定义 EnvOperation（如
非管理员降级方案），返回动作清单即可，无需对接事件机制。

## 测试参考

- `sdkcore` 测试：`null_sink()` + MockEnv（`tests/*.rs`）
- CLI 集成金样：根 `tests/cli_golden.rs`（Command 跑二进制 + SDKM_HOME 沙箱）——
  GUI 的行为基线与它一致：同一 core 数据 API，不同渲染端
