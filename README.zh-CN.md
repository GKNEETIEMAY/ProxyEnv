# 境启 ProxyEnv

> 面向 Windows 的轻量级环境变量管理工具；v0.1 聚焦代理环境变量一键启停与本机代理端口自动识别

**境启（ProxyEnv）** 是一个面向 Windows 的轻量级桌面工具，用于快速查看、启用和关闭代理相关环境变量，并自动识别本机正在运行的主流代理客户端及其实际监听端口。

它不替代 Clash、v2rayN、Hiddify 等代理软件，也不管理节点、订阅或路由规则。v0.1 先解决一个非常具体的问题：

> 有些程序必须依赖 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 才能联网，而另一些程序在这些环境变量存在时反而无法正常访问网络。

通过境启，你可以在启动目标程序之前临时关闭代理环境变量，启动完成后再恢复，不需要反复进入 Windows 环境变量设置页面手动删除和重建。

---

## 为什么需要境启

Windows 上不同软件使用代理的方式并不一致。

例如：

```text
浏览器
└─ Windows 系统代理
   └─ Clash / v2rayN
      └─ 正常访问

某些 CLI / API 工具
└─ HTTP_PROXY / HTTPS_PROXY
   └─ Clash / v2rayN
      └─ 正常访问

某些直连软件
└─ 检测到 HTTP_PROXY
   └─ 强制走代理
      └─ 访问失败
```

这会产生一个常见冲突：

```text
HTTP_PROXY 存在
├─ EasyCLIProxyAPI / CLI    ✅
└─ 某些直连软件             ❌

HTTP_PROXY 不存在
├─ EasyCLIProxyAPI / CLI    ❌
└─ 某些直连软件             ✅
```

境启 v0.1 的目标就是把原本繁琐的：

```text
打开 Windows 环境变量
→ 找到 HTTP_PROXY
→ 删除
→ 启动软件
→ 再重新添加
```

变成：

```text
关闭 Proxy Env
→ 启动软件
→ 恢复 Proxy Env
```

---

## 核心功能

### 一键启停代理环境变量

默认管理：

```text
HTTP_PROXY
HTTPS_PROXY
ALL_PROXY

http_proxy
https_proxy
all_proxy
```

同时展示：

```text
NO_PROXY
no_proxy
```

关闭时会真正删除对应变量，而不是写入空字符串。

开启时优先恢复关闭前保存的原始值。

---

## 自动识别本机代理

境启会尝试识别当前正在运行的代理软件以及实际监听端口。

首批重点兼容：

- Clash Verge Rev
- v2rayN
- FlClash
- Hiddify
- Clash Nyanpasu

同时兼容基于以下代理核心的其他客户端：

- Mihomo
- sing-box
- Xray

代理发现不会简单依赖默认端口，而是综合：

```text
Windows System Proxy
        ↓
代理客户端进程
        ↓
PID 与 TCP 监听端口
        ↓
HTTP / SOCKS5 协议探测
        ↓
客户端配置适配器
```

因此即使用户修改了 Clash 或 v2rayN 的默认端口，也能够尽量识别实际监听地址。

---

## 代理协议识别

支持识别：

```text
HTTP Proxy
SOCKS5 Proxy
Mixed Proxy
```

例如：

```text
Detected Proxy

Clash Verge Rev
127.0.0.1:7897
Mixed Proxy
● Listening
```

对于 Mixed Port，可生成：

```text
HTTP_PROXY=http://127.0.0.1:7897
HTTPS_PROXY=http://127.0.0.1:7897
ALL_PROXY=socks5://127.0.0.1:7897
```

---

## 使用方式

### 正常代理环境

```text
Proxy Environment

● ON
```

需要代理环境变量的软件正常启动。

### 临时关闭

准备启动一个不能继承代理环境变量的软件时：

```text
ProxyEnv
→ Disable
→ 启动目标软件
```

目标程序启动后：

```text
ProxyEnv
→ Enable
```

已经启动的软件通常保留其创建时获得的进程环境变量副本。

---

## 托盘操作

境启会提供 Windows 系统托盘入口。

示例：

```text
🟢 Proxy Env ON
```

左键快速切换：

```text
ON ↔ OFF
```

右键菜单：

```text
ProxyEnv
────────────────
● Proxy Env ON

Detected:
Clash Verge Rev
127.0.0.1:7897

Enable
Disable
Refresh
Open
Exit
```

---

## 不会做什么

境启不是代理客户端。

项目不会在 MVP 中实现：

- 节点管理
- 订阅管理
- Clash API 控制
- v2rayN 控制
- TUN 控制
- 修改 Windows System Proxy
- Per-App Proxy
- 自动启动其他程序
- 网络抓包
- VPN/TUN 驱动
- 按域名路由
- 代理核心内置

产品边界保持简单：

> **只管理代理环境变量，并帮助用户发现当前代理端口。**

---

---

## 长期方向

境启不会永远局限于代理环境变量。

在代理环境变量启停这一核心能力稳定之后，项目计划逐步扩展为通用 Windows 环境变量管理工具，包括：

```text
环境变量浏览与搜索
PATH 可视化、排序与失效项检测
变量快照与回滚
开发环境 Profile
JAVA_HOME / CUDA_PATH / Node / Python 等开发环境切换
API Base URL 等开发工具环境配置
```

设计原则是：

> **先把 Environment Core 做稳，再逐步增加管理能力；不为了未来功能牺牲 v0.1 的简单与可靠。**

## 技术栈

```text
Tauri 2
Vue 3
TypeScript
Rust
windows-rs
NSIS
GitHub Actions
```

目标平台：

```text
Windows 10 22H2 x64
Windows 11 x64
```

后续计划支持：

```text
Windows 11 ARM64
```

---

## 项目结构

```text
proxyenv/
├─ README.md
├─ README.zh-CN.md
├─ PRD.md
├─ TECHNICAL_DESIGN.md
├─ src/
├─ src-tauri/
└─ .github/
```

---

## 开发顺序

### Phase 1 — Environment Core

先实现：

```text
读取环境变量
→ Snapshot
→ Disable
→ Enable
→ WM_SETTINGCHANGE
→ Read-back Verification
```

### Phase 2 — Generic Proxy Detection

实现：

```text
Windows System Proxy
Process Scan
TCP PID Mapping
HTTP Probe
SOCKS5 Probe
Candidate Merge
Confidence Scoring
```

### Phase 3 — Client Identification

实现：

```text
Clash Verge Rev
v2rayN
FlClash
Hiddify
Clash Nyanpasu
```

### Phase 4 — UI & Tray

实现：

```text
Status
Toggle
Refresh
Detected Proxy
Warning
Tray
```

### Phase 5 — Release

实现：

```text
NSIS Installer
Portable ZIP
SHA256
GitHub Actions Release
```

---

## 开发原则

- 不需要管理员权限。
- 默认只修改 `HKCU\Environment`。
- 修改前必须保存环境变量快照。
- 禁止静默覆盖用户已有配置。
- 不硬编码代理客户端固定端口。
- 不扫描全部 1–65535 端口。
- 代理协议探测仅针对本机候选监听端口。
- 所有检测均在本地完成。
- 不收集、不上传代理配置、节点、订阅或 API Token。
- 主界面和托盘必须调用同一个 Rust Core。

---

## Release

计划提供：

```text
ProxyEnv_x.x.x_x64-setup.exe
ProxyEnv_x.x.x_x64-portable.zip
SHA256SUMS.txt
```

用户可以直接从 GitHub Releases 下载并运行。

---

## License

建议使用：

```text
MIT License
```

适合轻量开源工具，也方便社区贡献与二次开发。

---

## 项目定位

**境启 ProxyEnv**

> 从代理环境变量一键启停开始，逐步成为更简单、更可靠的 Windows 环境变量管理工具。
