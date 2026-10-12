# RouteSpan 发行安全

[English](release-security.md) · 简体中文

本文定义正式发行的信任模型，不宣称 QA 完成。维护者指定的上一版稳定基线是旧品牌 ProxyEnv v0.1.4，
GitHub Release 仍为 Draft；最新公开版为 v0.1.3。下一版 RouteSpan v0.2.0 尚未发布。
公开发布、证据与本地预览打包是不同事实，见[路线图](ROADMAP.zh-CN.md)和[验收台账](RELEASE_QA_V0.2.zh-CN.md)。

## 信任机制

| 机制 | 要求 |
| --- | --- |
| 锁定依赖 CI | 审计、构建和测试冻结的 npm/Cargo 依赖 |
| SHA-256 | 下载发行产物的校验值 |
| GitHub Artifact Attestation | 来源绑定预期仓库/工作流/tag/commit |
| Tauri Updater 签名 | 每个启用更新的正式发行必须提供 |
| 嵌入公钥 / 固定 HTTPS 来源 | 校验和端点属于受信任配置 |
| Windows Authenticode | 未采用，披露 Unknown Publisher/SmartScreen |

校验值可以发现字节变化，但本身不是签名。来源证明、更新校验和干净机器验收相互补充。
项目不要求付费证书/服务，但不能因此取消更新签名或手动验收。

## 兼容与更新

展示品牌为 RouteSpan。旧品牌 ProxyEnv 兼容标识包括内部 `proxyenv.exe`、`GKNEETIEMAY/ProxyEnv`、
应用身份、发行说明标记与更新来源：

```text
https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json
```

Logo/展示名改变不迁移这些标识。[内部契约](INTERNAL_CONTRACTS.zh-CN.md)列出受保护身份。

关于 → 检查更新读取固定 GitHub API/manifest。只有明确点击下载并安装才获取签名产物。
官方 Tauri Updater 用嵌入公钥校验，并保持仅升级比较规则。已注册 NSIS 安装可被动替换并重启；
MSI/便携版使用手动更新。不授权任意 URL、自定义下载器或执行未校验安装器。

## 正式发行流程

`.github/workflows/release.yml` 验证 tag/package/Cargo/Tauri 版本和多语言说明，
安装锁定依赖、审计/构建/测试、打包签名更新产物，收集安装器/便携程序、校验值和 `latest.json`，
为公开仓库产物生成来源证明，并创建 Draft Release。
正式 tag 仅接受稳定版本，拒绝 RC/预发行，而不更改更新器比较逻辑。
`pnpm test:release` 在打包前检查版本、兼容身份、四语说明与外部 `RouteSpan-…` 产物名称。
NSIS/MSI 签名保留配对名称，manifest 指向相同 NSIS；内部可执行文件为兼容保留 `proxyenv.exe`。

```text
锁定依赖检查 → 签名产物 → 校验值/来源证明
             → Draft → 外部验证与手动验收 → 发布
```

缺少私有签名 Secret、安装包签名或发行说明结构必须使正式流程失败。
不使用 `--no-sign` 或关闭更新产物来让正式发行通过。前端高危审计问题已修复，
`eee1873` 完整 CI 通过；每次发行变更仍必须审计。具体范围见验收台账，不能据此声称全部验收通过。

## 密钥处理

只提交公开校验公钥。CI 通过 Tauri 官方环境机制使用授权 Secrets
`TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。
受保护维护者恢复副本放在受跟踪源码之外。绝不打印、解码、打包或提交私有内容、前缀、长度、
密钥、密码或证书私有材料。

## 外部产物验证

每次预期发行需独立下载产物，并在[验收台账](RELEASE_QA_V0.2.zh-CN.md)保存证据：

```powershell
Get-FileHash -LiteralPath <artifact> -Algorithm SHA256
gh attestation verify <artifact> -R GKNEETIEMAY/ProxyEnv
```

确认校验值、预期仓库/工作流/tag/commit，及 Release 下载与对应 Actions 产物摘要一致。
验证 `latest.json` 引用已上传 NSIS，签名被已安装的可信应用接受。
绿色 CI 本身不是外部验证证据。

## 手动验收

记录干净 Windows 10/11 x64 安装、首次启动/卸载、前版升级保留身份/数据、便携版启动、
环境应用/恢复/冲突、单实例/托盘/任务栏/开始菜单/Alt+Tab 图标、125%/150% 原生 DPI、
主题和完整远程/重连/清理矩阵。

检查包内是否误含私钥、日志、本机配置、调试符号或开发文件。
如实说明 SmartScreen/Unknown Publisher；不能因为打包成功就宣称全部通过。

## 本地预览

手动预览可以用临时打包覆盖项关闭更新产物，不修改受跟踪发行配置。
标明预览、保留源码 SHA 和校验值，披露缺少更新签名/来源证明，不是正式更新版。
不得将未签名预览作为 `latest.json` 目标或静默升为稳定版。
2026-10-12 本地预览属于这种手动测试包。
