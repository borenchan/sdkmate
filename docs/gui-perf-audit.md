# sdkm-gui 体积与内存优化审计

> 测量环境：Windows 10 x86_64-msvc，rustc 1.98.1，MSVC 14.38，12 核。
> 所有数字均为本机实测；仓库外实验目录 `sdkmate-exp/`（不随仓库提交）。

## 一、结论摘要

| 指标 | 优化前 | 优化后 | 变化 | 目标 |
|---|---:|---:|---:|---|
| `sdkm-gui.exe` | 18.93 MiB | **14.26 MiB** | **−24.7%** | ≤15 MiB ✅ |
| `.text`（代码段） | 15.18 MiB | 10.65 MiB | −29.9% | — |
| `sdkm.exe`（CLI） | 5.02 MiB | 3.95 MiB | −21.4% | — |
| 冷启动（至可交互） | 156.3 ms | 155.9 ms | 持平 | 不牺牲 ✅ |
| 空闲 Private | 50.87 MB | 50.82 MB | 持平 | ≤40 MB ❌ |
| 空闲工作集 | 72.8 MB | 72.2 MB | −0.8 MB | — |
| 空闲线程数 | 11（峰值 22） | 11（峰值 11） | 峰值 −11 | — |
| GPU 本地显存 | 46.3 MB | 46.3 MB | 持平 | — |

**体积目标达成，内存目标未达成。** 未达成的原因不是缺少优化手段，而是 40 MB 落在框架地板之下——
在**完全无数据**（引导页、不加载任何 SDK 列表）的状态下，Private 就已经是 **43.36 MB**：

| 状态 | Private | 工作集 | GPU |
|---|---:|---:|---:|
| 空数据（引导页） | 43.36 MB | 42.78 MB | 11.7 MB |
| 加载真实 SDK 数据 | 50.8 MB | 72.2 MB | 46.3 MB |

这 43.36 MB 由 gpui 框架（DirectWrite 字体系统、D3D11 渲染、accesskit 无障碍、glyph atlas）产生，
应用代码在空闲态持有的数据（`sdk_rows`、页面状态、toast）合计远小于 1 MB。
要达到 40 MB，只能削减框架功能或换渲染后端，均已超出"不牺牲 UI 功能"的边界。

**推荐配置：全局 `opt-level = "s"`**（已落地）。全局 `"z"` 可再降到 12.57 MiB，但 `-Oz` 关闭向量化
与大部分内联，对布局/文本整形/图标栅格化有实际性能风险，而目标在 `"s"` 已达成，故不采纳。

## 二、基线事实

关键前提：**release profile 原本就已是重优化配置**（fat LTO + `codegen-units=1` + `panic="abort"` + `strip="symbols"`），
所以任务书里"strip / Thin LTO / Fat LTO / panic=abort"这几组并不是可优化项，而是既有基线。

| 项 | 值 |
|---|---|
| exe 文件 | 19,848,704 B = 18.93 MiB |
| `.text` | 15,922,926 B = 15.18 MiB（80%） |
| `.rdata` | 3,348,456 B = 3.19 MiB |
| `.data` / `.pdata` / `.rsrc` / `.reloc` | 136,704 / 203,408 / 131,784 / 102,736 B |
| 符号 | exe 内无 COFF 符号表，调试信息在独立 4.5 MB PDB |
| DLL 依赖 | 含 `VCRUNTIME140.dll`（需 VC++ 运行时；`+crt-static` 可去除但会增大体积） |
| 内嵌资源 | 22 个 SVG 9.5 KB + 128px PNG 24.9 KB + `sdkm.ico` 129 KB + gpui-kit 106 图标 47 KB + locales 24 KB ≈ 0.23 MB |
| 字体 | 仓库内**无任何字体文件**，全部走系统字体与 OS 回退 |
| 1830 图标全量包 | 已核实**未链入**（抽样 40 个非默认图标，0 命中） |

## 三、体积归因

MSVC 产物无符号表（只有 PDB），`cargo-bloat` 无法直接读；改用**链接 map 文件**做精确归因：
`link.exe /MAP` 输出 7 万余符号，按地址差计算每个符号的字节数，`rustfilt` 反修饰后按 crate 聚合。
误差校验：聚合总量 15.19 MiB vs `.text` 实测 15.18 MiB，**误差 <1%**。

### `.text` 按 crate（前 24）

| MiB | 占比 | crate | MiB | 占比 | crate |
|---:|---:|---|---:|---:|---|
| 2.40 | 15.8% | gpui | 0.27 | 1.8% | taffy |
| 1.25 | 8.2% | core（std core） | 0.26 | 1.7% | alloc |
| 1.19 | 7.9% | gpui_base | 0.23 | 1.5% | std |
| 0.73 | 4.8% | gpui_component | 0.23 | 1.5% | gpui_windows |
| 0.60 | 3.9% | usvg | 0.22 | 1.4% | exr |
| 0.51 | 3.4% | **sdkm_gui（业务 UI）** | 0.21 | 1.4% | rustybuzz |
| 0.48 | 3.2% | zune_jpeg | 0.21 | 1.4% | markdown |
| 0.36 | 2.3% | image_webp | 0.19 | 1.2% | hashbrown |
| 0.34 | 2.3% | rustls | 0.18 | 1.2% | tiny_skia |
| 0.32 | 2.1% | image | 0.16 | 1.0% | resvg |
| 0.32 | 2.1% | regex_automata | 0.15 | 1.0% | aho_corasick |
| 0.30 | 2.0% | **sdkm_core（业务逻辑）** | 0.15 | 1.0% | regex_syntax |

### 按功能分组

| 分组 | 体积 | 占比 | 说明 |
|---|---:|---:|---|
| gpui / gpui_component / std / 业务代码 | 10.33 MiB | 68.0% | 框架主体，不可裁 |
| 图像解码（image 全家） | 1.63 MiB | 10.8% | `gpui-pre` 强制开启全部编解码 feature（bmp/dds/exr/gif/hdr/ico/jpeg/png/pnm/qoi/tga/tiff/webp），我方无法关闭 |
| SVG 渲染（usvg/resvg/tiny-skia） | 1.08 MiB | 7.1% | 图标渲染必需 |
| 文字整形 / 字体 | 0.72 MiB | 4.7% | rustybuzz + ttf-parser |
| 正则 | 0.62 MiB | 4.1% | 间接依赖 |
| markdown / 编辑器栈 | 0.41 MiB | 2.7% | gpui-component 非可选依赖（含 lsp-types、html5ever、ropey） |
| 网络 / 加密 | 0.40 MiB | 2.6% | reqwest + rustls |
| 合计 | 15.19 MiB | 100% | |

结论：**业务代码（sdkm_gui + sdkm_core）只占 0.81 MiB**，剩下 94% 是框架与依赖。
体积只能靠编译器优化等级压制，不能靠删业务代码。

## 四、编译实验矩阵（逐项实测）

所有变体独立构建，记录文件大小、`.text`、构建耗时；冷启动与 RSS 见第五节。

| # | 变体 | exe | MiB | `.text` | 构建耗时 | 结论 |
|---|---|---:|---:|---:|---:|---|
| V0 | 基线 `opt-level=3` | 19,848,704 | 18.93 | 15.18 MiB | 292 s | 基线 |
| V1 | 冷依赖 `opt-level="z"`（叠加在 `s` 上） | 14,938,112 | 14.25 | 10.61 MiB | 286 s | 仅比 `s` 少 35 KB，**不采纳**（fat LTO 稀释了 per-package 效果，不值配置复杂度） |
| V2 | **全局 `opt-level="s"`** | 14,973,440 | **14.28** | 10.66 MiB | 359 s | **采纳为主方案** |
| V3 | 全局 `opt-level="z"` | 13,178,880 | 12.57 | 8.86 MiB | 313 s | 数据点；再省 1.71 MiB，但 `-Oz` 性能风险不可接受 |
| V4 | V2 + core 单线程 runtime + 裁 tokio feature + `miniz_oxide` 保 O3 | 14,948,352 | **14.26** | 10.65 MiB | 279 s | **最终采用** |

### 未实测、按证据排除的项

| 项 | 结论与依据 |
|---|---|
| 显式 `/OPT:REF`、`/OPT:ICF` | rustc 本就传这两个参数，重复添加是 no-op |
| `/Gy`、`/Gw` | cl.exe 编译期开关，Rust COFF 已是函数级 COMDAT |
| `strip = true` / `"debuginfo"` | `true` 与现有 `"symbols"` 等价；MSVC 调试信息在 PDB，改 strip 无收益 |
| `panic = "abort"` | 已开启。**代价需记录**：gpui 内部有 `catch_unwind`，abort 下这些捕获失效，UI panic 会直接终止进程（现状如此，非本次引入） |
| `lto = "thin"` | 未实测。fat LTO 的全局 DCE/内部化强于 thin，thin 通常更大；基线已是最优形态 |
| 关闭 `gpui-kit` 的 `assets` feature | `gpui-component` 对 `gpui-kit-assets` 是**非可选依赖**，关掉省 0 字节且 `assets.rs` 编译失败 |
| `lld-link` | 尝试失败（rustup 的 `gcc-ld/lld-link.exe` 不接受 rustc 传入的 `-flavor`），未取得体积数据；目标已达成，不再追 |
| `+crt-static` | 会增大约 0.1~0.4 MB，只作为"去 VCRUNTIME140 依赖"的移植性选项，本次不采纳 |
| UPX | 本机无 UPX，未实测。已知代价：AV/SmartScreen 误报、破坏数字签名、启动全量解压、崩溃转储失效；**明确不采纳** |
| `cargo-bloat` | MSVC 无符号表，预期读不出；改用链接 map，精度更高 |

## 五、内存测量

### 指标定义（重要）

- **头条指标 = `PrivateMemorySize64`**（私有提交），与"任务管理器内存"口径接近。
- `WorkingSet64` 仅作参考：包含共享 DLL 与文件映射页，跨机器不可比。
- **GPU 显存单列**（`GPUProcessMemory.LocalUsage`），不混入 RSS。
- 不做 `EmptyWorkingSet` 之类把数字做漂亮的操作。

### 采样点

启动后 → 首屏加载完成 → 空闲 60 s → 五页各走一遍 → 两轮页面切换 → 触发一次列表刷新（250 ms 采样取峰值）→ 任务结束 30 s。

### 结果

| 变体 | 冷启动中位数 | 空闲 Private | 空闲工作集 | 空闲线程 | 峰值线程 | 峰值 Private |
|---|---:|---:|---:|---:|---:|---:|
| 基线（opt3 + 多线程 runtime） | 156.3 ms | 50.87 MB | 72.8 MB | 11 | **22** | 71.51 MB |
| `opt-level="s"` | 154.5 ms | 50.73 MB | 72.0 MB | 11 | 11 | 68.9 MB |
| `opt-level="z"` | 155.1 ms | 50.84 MB | 72.0 MB | 11 | 11 | — |
| 最终版（s + 单线程 runtime） | 155.9 ms | 50.82 MB | 72.2 MB | 11 | 11 | 68.91 MB |
| 空数据（框架地板） | — | **43.36 MB** | 42.78 MB | 11 | — | — |

### 为什么空闲 RSS 降不下去

1. **框架地板 43.36 MB**：DirectWrite 字体集合、D3D11 设备与交换链、accesskit、glyph atlas（1024² 三张 atlas 约 9 MB，属显存）在空窗口时即已分配。
2. **加载数据后 +7.4 MB Private / +34 MB GPU**：SDK 图标栅格化与更丰富的界面绘制。
3. 应用层持有量（`sdk_rows`、页面状态、toast）合计 <1 MB，不是矛盾点。
4. 空闲帧间隔（`inactive_frame_interval`）只节流待处理重绘，GPUI 是脏标记驱动，**对 RSS 无影响**，属 CPU/耗电项。

## 六、本次落地的改动

### 1. 编译配置（根 `Cargo.toml`）

```toml
[profile.release]
opt-level = "s"      # 体积优先：GUI 代码段占 80%
lto = true
codegen-units = 1
panic = "abort"
strip = "symbols"

# 解压是纯 CPU 热点（纯 Rust miniz_oxide），保持 -O3，抵消全局 opt-level=s 带来的吞吐损失
[profile.release.package.miniz_oxide]
opt-level = 3
```

### 2. sdkm-core：单线程 tokio runtime

原来 5 处同步桥接各自 `tokio::runtime::Runtime::new()`（多线程，每核一个 worker）；GUI 每个后台任务
都会构造一次，启动时并发 2~6 个任务 → 峰值线程 22。core 的异步流程全是顺序 await，
没有 `tokio::spawn` / `join`，单线程 runtime 完全够用。

- 新增 `crates/sdkm-core/src/runtime.rs`（`new_runtime()` → `Builder::new_current_thread().enable_all()`）
- `install/mod.rs`（2 处）、`list.rs`（1 处）、`self_update.rs`（3 处）改用该 helper
- workspace `tokio` feature：`rt-multi-thread` → `rt` + `sync`
- 测试同步改为 current-thread runtime（`crates/sdkm-core/tests/install.rs`）

效果：峰值线程 22 → 11，峰值 Private 71.5 → 68.9 MB；**空闲 RSS 不变**（runtime 用完即释放，这点必须如实说明）。

### 3. 验证

- `cargo test -p sdkm-core -p sdkm-cli`：**102 项全绿**
- `cargo fmt --all --check`、`cargo clippy`（无新增告警）
- 真实网络请求走新 runtime：`sdkm list java --remote --limit 3` 正常返回
- `sdkm ls` 热态 19~20 ms（与旧构建持平）
- 冷启动、五页导航、图标/CJK 渲染冒烟通过

## 七、未做但值得关注

| 项 | 潜在收益 | 成本/风险 |
|---|---|---|
| 全局 `opt-level="z"` | −1.7 MiB（→12.57 MiB） | `-Oz` 关闭向量化/大部分内联，布局与栅格化变慢；需逐帧性能基准才能定论 |
| 依赖去重（`[patch.crates-io]`） | 约 −0.5~2 MiB | 同时存在两套 `resvg/usvg`（0.45/0.46）、两份 `windows`（0.58/0.62）、两份 `png`；需 vendor 上游改依赖，长期维护成本高 |
| 裁剪 `image` 编解码 | 约 −1.5 MiB（只留 png/ico） | `gpui-pre` 硬编码全量 feature，必须 fork 上游 |
| `+crt-static` | 体积 +0.1~0.4 MB | 换来不依赖 VC++ 运行时的绿色分发 |
| 减少 SDK 图标栅格化 | 峰值 GPU −10~20 MB | 属产品视觉决策，且只影响显存不影响 RSS |
| `inactive_frame_interval` | 空闲 CPU/耗电 | 与 RSS 无关，本次未纳入 |

## 八、复现方式

```powershell
# 基线
cargo build --release -p sdkm-gui --locked
objdump -h target/release/sdkm-gui.exe

# 归因（MSVC 链接 map）
cargo rustc -p sdkm-gui --release --bin sdkm-gui --locked -- -C link-arg=/MAP:D:\path\sdkm-gui.map
# 再用 rustfilt 反修饰，按地址差聚合

# 变体
# 编辑根 Cargo.toml 的 opt-level 后重新 build，记录字节数
```

内存采样脚本与原始 CSV 位于仓库外 `sdkmate-exp/`（`scripts/measure.ps1`、`scripts/peak.ps1`、`logs/`），
不随仓库提交。