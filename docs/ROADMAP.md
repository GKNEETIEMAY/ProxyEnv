# ProxyEnv Roadmap

```yaml
Current Stable: v0.1.3
Next: v0.1.4
```

This is the release-scope reference for project documentation. In **Current**, checked items are shipped capabilities. In **Next**, checked items mean implemented in the development tree, **not released**; unchecked items remain pending. Scope may change after review and Windows acceptance. No release date or Linux/macOS implementation schedule is promised.

本文是文档发布范围的统一依据：Current 勾选项表示已发布；Next 勾选项仅表示开发树已实现，**不代表已发版或已完成安装版验收**。未勾选项仍待完成，范围可随评审调整，不承诺发版日期或 Linux/macOS 实现时间。

## Current — v0.1.3

- [x] Proxy discovery — client/process grouping, actual listening ports and protocol detection / 代理发现、客户端归并、监听端口与协议检测。
- [x] Environment management — selected user variables, snapshots, broadcast, verified restore and conflict protection / 环境变量管理、快照、广播、恢复与冲突保护。
- [x] Independent Windows System Proxy and TUN observation / 独立的系统代理与虚拟网卡只读观测。
- [x] Application Assistant — application selection, diagnosis, explicit proxy/direct launch and confirmed manual-guide restart / 应用助手、显式启动与确认重启。
- [x] Protected application-rule engine and schema; **no reviewed production rules bundled yet** / 已有受保护规则引擎和 Schema，尚无已评审生产规则。
- [x] Signed updater — user-triggered check and download/install for NSIS; MSI/Portable update manually / NSIS 签名更新，MSI/Portable 手动更新。

## Next — v0.1.4

- [ ] Application Network Profile / 应用网络 Profile。
- [x] Unified ActiveProxyContext — explicit shared selection, unavailable state and stale-operation guards / 全局活动代理、失效保留与旧操作保护；已实现、未发布。
- [x] Conservative application diagnosis — environment configuration does not prove application connectivity / 保守诊断状态，不凭环境变量断言应用联网；已实现、未发布。
- [ ] Initial reviewed application rules — validate real application versions, fields and restore behavior before bundling / 首批已评审规则，须验证真实版本、字段与恢复行为。
- [x] Safe diagnostic report — preview, four output languages, copy, allowlisted redaction and no new network probes / 安全诊断报告；已实现并完成本地测试、未发布。
- [x] Focused frontend report tests and CI integration / 报告格式、多语言及边界测试已接入 CI；不代表完整前端测试覆盖。
- [ ] Broader frontend interaction tests and packaged Windows 10/11 acceptance / 扩展交互测试与 Windows 10/11 安装版验收。
- [ ] Release QA evidence: signed update/installation, checksums and Artifact Attestation verification / 补齐签名更新、安装、校验和及 Attestation 发行证据。

## Future

- More reviewed application knowledge and stronger evidence-based diagnosis / 更多经过验证的应用知识与更完善的诊断策略。
- Community-contributed Profiles/Rules with maintainer review and safety validation; no automatic rule marketplace / 经维护者评审和安全验证的社区 Profile/Rule，不自动下载市场规则。
- Explore Linux/macOS platform adapters without committing to an implementation date; other Unix variants remain out of scope / 探索 Linux/macOS 平台适配，不承诺实现时间；其他 Unix 变体不在范围内。
