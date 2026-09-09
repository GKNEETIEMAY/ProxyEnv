# ProxyEnv Roadmap

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

This is the release-scope reference for project documentation. Checked items under **Current** are shipped capabilities. **Next** describes intended work and may change after review and Windows acceptance. No release date or Linux/macOS implementation schedule is promised.

本文是项目文档的发布范围依据。**Current** 中的勾选项表示已发布能力；**Next** 表示计划中的工作，范围可能随评审和 Windows 验收调整。不承诺发版日期或 Linux/macOS 实现时间。

## Current — v0.1.4

- [x] Proxy discovery — client/process grouping, actual listening ports, protocol detection and shared client icons / 代理发现、客户端归并、实际监听端口、协议检测与通用图标。
- [x] Environment management — selected user variables, snapshots, broadcast, verified restore and conflict protection / 环境变量管理、快照、广播、恢复与冲突保护。
- [x] Independent Windows System Proxy and TUN / virtual-adapter observation / 独立的系统代理与虚拟网卡只读观测。
- [x] Unified ActiveProxyContext — one explicit proxy selection shared by synchronization, mismatch checks, connectivity tests, the assistant and application launches / 全局活动代理统一用于同步、Mismatch、连通性测试、应用助手与应用启动。
- [x] Conservative Application Assistant diagnosis — configured environment variables do not prove application connectivity / 应用助手采用保守诊断，不凭环境变量断言应用联网。
- [x] Explicit proxy/direct application launches and confirmed manual-guide restart / 显式代理或直连启动，以及经过二次确认的手动引导重启。
- [x] Safe multilingual diagnostic reports — redacted preview, refresh and copy in Simplified Chinese, English, Japanese and Korean / 支持四种语言的脱敏诊断报告预览、刷新与复制。
- [x] Focused diagnostic-report tests and CI integration / 诊断报告格式、多语言及无网络探测边界测试已接入 CI。
- [x] Signed updater — user-triggered check and download/install for NSIS; MSI/Portable update manually / NSIS 签名更新，MSI/Portable 手动更新。

## Next — v0.2.0 Remote Environment Bridge

Implementation is on `v0.2.0dev`; the stable release remains v0.1.4 pending live Windows/SSH acceptance. See [implementation and compatibility notes](REMOTE_BRIDGE.md).

- [x] Separate first-level Local Environment and Remote Bridge pages with a shared global Settings surface.
- [x] Structured remote targets from local OpenSSH, VS Code Remote SSH configuration, and safely convertible MobaXterm bookmarks.
- [x] Source-aware connection behavior, explicit SSH checks, and VS Code actions only for compatible targets.
- [x] Current ActiveProxyContext reverse forwarding, revision protection, and session lifecycle.
- [x] Automatically allocated remote loopback ports with a second occupancy check before connection.
- [x] CC Switch route detection that distinguishes confirmed ownership, an unknown listener, and no listener.
- [x] Structured bridge errors, a bridge-health panel, and capability-specific next-step guidance.
- [x] Dedicated Codex/Claude CLI overlays with reviewed writes, remote backups, and conflict-safe restore.
- [x] Shared RemoteToolAdapter registry for Codex/Claude CLI metadata, compatibility, lifecycle operations, launch commands, and honest pending-verification state.
- [x] User-triggered Claude CLI request verification with isolated settings/tools, redacted result categories, and verification invalidation when the route changes.
- [x] Claude Code Remote - SSH preview/apply/restore support for `environmentVariables` plus explicitly reviewed `disableLoginPrompt`.
- [x] Four-language Remote Bridge interface and redacted diagnostics.
- [ ] Live Windows → Linux SSH, MobaXterm compatibility, CC Switch routing, Claude request verification, and Codex request acceptance on user-provided targets.
- [ ] Opt-in Codex / Claude Code VS Code extension acceptance, separate from CLI scope, with user-confirmed remote execution location.

实现已进入 `v0.2.0dev`，仍需真实 Windows/SSH 环境验收；尚未作为稳定版发布。

图形化扩展适配已进入开发分支：可分别选择 CLI 与扩展，确认远端运行位置后预览、应用或恢复配置。结构化目标、自动端口、CC Switch 分级检测和桥接后指引已经实现，但两种 CLI、两种扩展、Provider 切换与断连失败链路全部真实验收后，才可声明稳定支持。详见[扩展审计](REMOTE_BRIDGE_EXTENSION_AUDIT.md)。

## Future

- More verified application knowledge and clearer diagnosis explanations without automatically changing third-party application configuration / 增加经过验证的应用知识与更清晰的诊断说明，但不自动修改第三方应用配置。
- Explore Linux/macOS platform adapters without committing to an implementation date; other Unix variants remain out of scope / 探索 Linux/macOS 平台适配，不承诺实现时间；其他 Unix 变体不在范围内。

ProxyEnv's product boundary remains deliberately narrow: make the Windows proxy environment visible, consistent and controllable, then help users launch applications with an explicit proxy or direct environment. Third-party application configuration maintenance is not a release-roadmap focus.

ProxyEnv 保持明确的产品边界：让 Windows 代理环境变得可见、统一、可控，并帮助用户以明确的代理或直连环境启动应用；长期维护第三方应用内部配置不属于当前发行路线重点。
