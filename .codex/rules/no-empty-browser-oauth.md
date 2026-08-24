# 禁止空浏览器做 OAuth / 登录

**不得**为 OAuth、Google 授权、扫码登录等身份验证，启动「空浏览器」或自动化浏览器实例。

## 禁止

- Playwright / Puppeteer / Selenium 启动的**全新 Chromium**（无用户 Cookie、无已登录会话）
- `chromium.launch()`、`playwright.chromium.launch()` 等无 Profile 的浏览器自动化
- 批量重试、并行多标签触发 OAuth（会导致 state 冲突、授权码无法落盘）
- 用空浏览器「代用户登录 Google」——空会话无法自动完成账号密码登录

## 允许

- **MCP Router 桌面应用**里的 Authenticate / 官方 OAuth 流程
- 用 `open_resource` 或系统默认浏览器**打开一次**授权 URL，由用户在**已登录的浏览器**里点继续
- 用户明确要求的、且使用**其 Chrome/Edge 用户 Profile**（含 Cookie）的自动化——须先说明风险并获同意
- 只读验证 token 是否已存在、检查 credentials 目录、重启 MCP 服务——不涉及开浏览器

## OAuth 执行原则

1. **单次流程**：同一账号只触发一次 `start_google_auth`，等回调完成后再验证
2. **不并行**：不同时跑多个 OAuth 脚本、不重复杀进程后立即再开 auth
3. **阻塞才说明**：仅当必须用户在本机已登录浏览器中点「允许」时，说明打开哪个链接、等什么结果；不把「请你运行脚本 / 装 Playwright」甩给用户

与 `agent-execute-not-delegate.mdc` 配合：Agent 仍应自己检查 Router、凭证文件、端口与日志；**禁止用空浏览器冒充「已完成授权」。**
