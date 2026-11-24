# sdkm 文档

按读者分组。只想快速跑起来，先看 [项目 README 的 Quick Start](../README.md)；完整用法从 [usage.md](./usage.md) 开始。

## 使用者

| 文档 | 内容 |
|:---|:---|
| [usage.md](./usage.md) | 详细用法入口：项目级版本管理、Shell hook、交互式 TUI、工作原理 |
| [commands.md](./commands.md) | 每个子命令的参数、别名、行为与示例 |
| [configuration.md](./configuration.md) | `config.toml` 结构与每个配置项的含义、校验规则 |
| [custom-sdk.md](./custom-sdk.md) | 把任意工具注册为自定义 SDK；42 个内置 SDK 名单 |
| [sdk-onboarding.md](./sdk-onboarding.md) | 什么工具适合接入 sdkm；用户自助注册 vs 贡献内置 |
| [gui.md](./gui.md) | 桌面应用 `sdkm-gui` 的页面与操作 |
| [faq.md](./faq.md) | 常见问题排查 |

## 贡献者

| 文档 | 内容 |
|:---|:---|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | 协作流程、提交与 PR 规范、工程约束 |
| [development.md](./development.md) | 构建、分层架构、测试沙箱、质量门禁、CLI 命令落位 |
| [development-notes.md](./development-notes.md) | 易变开发注意事项：历史坑、跨层契约与平台约束 |
| [sdk-onboarding.md](./sdk-onboarding.md) | 新增内置 SDK 的准入门槛与实现路径 |

> 维护约定：易变内容写 [development-notes.md](./development-notes.md)，不要往 `CLAUDE.md` 追加；目录按读者分组，但文件保持平铺，避免断链。