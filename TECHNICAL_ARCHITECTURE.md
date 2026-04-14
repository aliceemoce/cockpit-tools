# Cockpit Tools 技术架构文档

> 本文档详细解析 Cockpit Tools 的技术实现、架构设计和核心功能模块。

## 项目概览

**项目名称**: Cockpit Tools  
**版本**: v0.21.1  
**定位**: 通用 AI IDE 账号管理工具  
**许可协议**: CC BY-NC-SA 4.0（署名-非商业性使用-相同方式共享）  

### 支持平台

| 平台 | 账号管理 | 多开实例 | OAuth登录 |
|------|---------|---------|----------|
| Antigravity | ✅ | ✅ | ✅ |
| Codex | ✅ | ✅ | ✅ |
| GitHub Copilot | ✅ | ✅ | ✅ |
| Windsurf | ✅ | ✅ | ✅ |
| Kiro | ✅ | ✅ | ✅ |
| Cursor | ✅ | ✅ | ✅ |
| Gemini CLI | ✅ | ❌ | ✅ |
| CodeBuddy | ✅ | ✅ | ✅ |
| CodeBuddy CN | ✅ | ✅ | ✅ |
| Qoder | ✅ | ✅ | ✅ |
| Trae | ✅ | ✅ | ✅ |
| Zed | ✅ | ❌ | ✅ |

---

## 技术栈

### 前端技术栈

| 技术 | 版本 | 用途 |
|-----|------|------|
| **React** | 19.1.0 | UI 框架 |
| **TypeScript** | 5.8.3 | 类型系统 |
| **Vite** | 7.0.4 | 构建工具 |
| **TailwindCSS** | 3.4.19 | CSS 框架 |
| **DaisyUI** | 5.5.14 | 组件库 |
| **Zustand** | 5.0.10 | 状态管理 |
| **i18next** | 25.7.4 | 国际化 |
| **Lucide React** | 0.562.0 | 图标库 |
| **Tauri API** | ^2 | 桌面端桥接 |

### 后端技术栈

| 技术 | 版本 | 用途 |
|-----|------|------|
| **Rust** | Edition 2021 | 后端语言 |
| **Tauri** | 2.x | 桌面应用框架 |
| **Tokio** | 1.x | 异步运行时 |
| **SQLite** | bundled | 本地数据库 |
| **Reqwest** | 0.12 | HTTP 客户端 |
| **Tokio-Tungstenite** | 0.26 | WebSocket |
| **RSA/AES-GCM** | - | 加密算法 |
| **Tracing** | 0.3 | 日志系统 |

---

## 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        Cockpit Tools                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   React UI   │  │  Zustand     │  │  i18next     │          │
│  │   (Frontend) │  │  (State)     │  │  (i18n)      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
├─────────────────────────────────────────────────────────────────┤
│                      Tauri Bridge Layer                         │
│         (IPC Commands + Events + File System)                  │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Commands   │  │   Modules    │  │   Models     │          │
│  │   (API)      │  │  (Business)  │  │   (Data)     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
├─────────────────────────────────────────────────────────────────┤
│                      Rust Core Layer                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│  │ Account │ │ OAuth   │ │ Instance│ │ WebSocket│ │  Quota  │  │
│  │ Module  │ │ Module  │ │ Module  │ │ Module  │ │ Module  │  │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                    External Services                            │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│  │Antigravity│ │  Codex  │ │  Copilot│ │  Cursor │ │  Trae   │  │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 目录结构

```
fork-cockpit-tools/
├── src/                          # 前端代码
│   ├── App.tsx                   # 主应用组件
│   ├── main.tsx                  # 入口文件
│   ├── components/               # 通用组件
│   ├── pages/                    # 页面组件
│   ├── stores/                   # Zustand 状态管理
│   ├── services/                 # API 服务层
│   ├── hooks/                    # 自定义 Hooks
│   ├── types/                    # TypeScript 类型定义
│   ├── i18n/                     # 国际化配置
│   ├── locales/                  # 18种语言翻译文件
│   └── utils/                    # 工具函数
│
├── src-tauri/                    # Tauri 后端代码
│   ├── src/
│   │   ├── lib.rs                # 应用入口
│   │   ├── main.rs               # 主函数
│   │   ├── commands/             # 36个命令模块 (IPC API)
│   │   ├── modules/              # 84个业务模块
│   │   ├── models/               # 数据模型定义
│   │   └── utils/                # 工具模块
│   ├── Cargo.toml                # Rust 依赖配置
│   └── tauri.conf.json           # Tauri 应用配置
│
├── scripts/                      # 构建脚本
├── docs/                         # 文档和图片
├── public/                       # 静态资源
└── .github/workflows/            # CI/CD 工作流
```

---

## 前端架构详解

### 1. 组件架构

采用**分层组件设计**：

```
┌─────────────────────────────────────────┐
│           Page Components               │
│  (Dashboard, Accounts, Settings...)    │
├─────────────────────────────────────────┤
│         Feature Components              │
│  (InstancesManager, MfaVaultManager)   │
├─────────────────────────────────────────┤
│          UI Components                  │
│  (Modal, Dropdown, TabsHeader)         │
├─────────────────────────────────────────┤
│          Layout Components              │
│  (SideNav, FloatingCardWindow)         │
└─────────────────────────────────────────┘
```

### 2. 状态管理 (Zustand)

采用**按平台分离**的 Store 设计：

```typescript
// 示例：createProviderAccountStore 工厂函数
export function createProviderAccountStore<T extends BaseAccount>(
  provider: Provider,
  options: StoreOptions
) {
  return create<AccountStore<T>>()(
    persist(
      (set, get) => ({
        accounts: [],
        currentAccountId: null,
        // ... actions
      }),
      { name: `cockpit-${provider}-accounts` }
    )
  );
}
```

**Store 列表** (33个)：
- `useAccountStore` - Antigravity 账号
- `useCodexAccountStore` - Codex 账号
- `useGitHubCopilotAccountStore` - Copilot 账号
- `useCursorAccountStore` / `useWindsurfAccountStore` / etc.
- `usePlatformLayoutStore` - 平台布局配置 (44KB，核心配置)

### 3. 懒加载优化

```typescript
// App.tsx 中所有页面均采用懒加载
const DashboardPage = lazy(() =>
  import('./pages/DashboardPage').then(m => ({ default: m.DashboardPage }))
);
// ... 20+ 个页面组件
```

### 4. 国际化架构

支持 **18 种语言**：
- 核心语言：英语、简体中文、繁體中文、日本語
- 欧洲：Deutsch、Español、Français、Italiano、Polski、Čeština
- 其他：한국어、Português、Русский、Türkçe、العربية、Tiếng Việt、Bahasa Indonesia

```typescript
// i18n/index.ts
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: enTranslations },
    'zh-CN': { translation: zhCNTranslations },
    // ... 18 languages
  },
  lng: 'zh-CN',
  fallbackLng: 'en',
});
```

---

## 后端架构详解

### 1. 模块组织

| 模块类别 | 数量 | 核心文件 |
|---------|------|---------|
| Commands | 36 | `commands/` 目录 |
| Business Modules | 84 | `modules/` 目录 |
| Data Models | 16 | `models/` 目录 |

### 2. IPC Commands 架构

按功能域分组的命令处理器：

```rust
// lib.rs: invoke_handler 配置
tauri::generate_handler![
    // Account Commands (24个)
    commands::account::list_accounts,
    commands::account::add_account,
    commands::account::switch_account,
    // ...
    
    // Platform Commands (每平台8-15个)
    commands::codex::list_codex_accounts,
    commands::codex::switch_codex_account,
    commands::cursor::inject_cursor_account,
    // ...
    
    // Instance Commands (每平台9个)
    commands::codex_instance::codex_list_instances,
    commands::codex_instance::codex_start_instance,
    // ...
    
    // System Commands (40+个)
    commands::system::open_data_folder,
    commands::system::save_general_config,
    // ...
]
```

### 3. 核心业务模块

| 模块 | 功能 | 代码规模 |
|-----|------|---------|
| `process.rs` | 进程管理 | 306KB |
| `macos_native_menu.rs` | macOS 原生菜单 | 160KB |
| `tray.rs` | 系统托盘 | 122KB |
| `codex_account.rs` | Codex 账号管理 | 121KB |
| `trae_account.rs` | Trae 账号管理 | 100KB |
| `web_report.rs` | 网页报告服务 | 84KB |
| `wakeup.rs` | 唤醒任务核心 | 80KB |
| `codex_wakeup.rs` | Codex 唤醒 | 77KB |
| `wakeup_scheduler.rs` | 任务调度器 | 36KB |
| `kiro_oauth.rs` | Kiro OAuth | 88KB |
| `cursor_instance.rs` | Cursor 实例管理 | 47KB |
| `windsurf_instance.rs` | Windsurf 实例 | 66KB |

### 4. WebSocket 服务架构

```rust
// modules/websocket.rs
pub async fn start_server() {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();
    
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }
}

async fn handle_connection(stream: TcpStream) {
    let ws_stream = accept_async(stream).await.unwrap();
    // 处理客户端消息...
}
```

**功能**：为浏览器插件提供本地通信接口  
**默认端口**：19528 (可配置)  
**安全**：仅监听 127.0.0.1，无远程访问风险

### 5. OAuth 实现架构

多平台 OAuth 统一设计：

```rust
// 每个平台独立的 OAuth 模块
modules::codex_oauth
modules::windsurf_oauth
modules::kiro_oauth
modules::trae_oauth
// ...

// 启动时恢复挂起的 OAuth 状态
tauri::async_runtime::spawn(async move {
    modules::codex_oauth::restore_pending_oauth_listener(app_handle);
    modules::windsurf_oauth::restore_pending_oauth_listener();
    // ...
});
```

---

## 数据存储架构

### 1. 存储位置

| 数据类型 | 存储路径 | 说明 |
|---------|---------|------|
| 应用配置 | `~/.config/cockpit-tools/` (Linux) | Tauri 标准目录 |
| Antigravity 账号 | `~/.antigravity_cockpit/` | 独立目录 |
| Codex 本地 | `~/.codex/auth.json` | 官方路径兼容 |
| Gemini CLI | `~/.gemini/` | 官方路径兼容 |
| SQLite 数据库 | 应用数据目录 | rusqlite |
| 日志文件 | 应用数据目录/logs/ | tracing-appender |

### 2. 数据模型

```rust
// models/account.rs
pub struct Account {
    pub id: String,
    pub email: String,
    pub token: TokenData,
    pub quota: Option<QuotaData>,
    pub tags: Vec<String>,
    pub device_fingerprint: Option<String>,
    pub created_at: i64,
}

pub struct QuotaData {
    pub credits: CreditInfo,
    pub reset_time: Option<chrono::DateTime<chrono::Utc>>,
}
```

### 3. 加密策略

```rust
// Windows VS Code Copilot 令牌注入使用 AES-GCM
#[cfg(target_os = "windows")]
use aes_gcm::Aes256Gcm;
use aes::Aes128;
use cbc::CbcDecrypt;
use pbkdf2::pbkdf2_hmac;
```

---

## 实例管理架构

### 多开实例设计

支持多平台的**独立实例**运行：

```rust
// 实例配置结构
pub struct InstanceProfile {
    pub id: String,
    pub name: String,
    pub provider: String,        // "codex" | "cursor" | "windsurf"...
    pub account_id: String,
    pub user_data_dir: PathBuf,  // 独立用户目录
    pub launch_args: Vec<String>, // 启动参数
    pub env_vars: HashMap<String, String>, // 环境变量
}

// 实例生命周期管理
pub enum InstanceState {
    Stopped,
    Starting,
    Running { pid: u32 },
    Stopping,
    Error(String),
}
```

### 实例隔离机制

| 平台 | 隔离方式 | 实现方式 |
|-----|---------|---------|
| VS Code / Copilot | 用户目录隔离 | `--user-data-dir` |
| Cursor | 用户目录 + 进程隔离 | 自定义启动器 |
| Windsurf | 用户目录隔离 | 参数注入 |
| Codex | 环境变量 + 目录隔离 | `CODEX_CONFIG_DIR` |
| Trae / Kiro | 用户目录隔离 | 启动参数 |

---

## 网络服务架构

### 1. 内置 HTTP 服务

```rust
// modules/web_report.rs
pub async fn start_server() {
    tiny_http::Server::http("127.0.0.1:0").map(|server| {
        for request in server.incoming_requests() {
            handle_request(request);
        }
    });
}
```

**用途**：提供本地查询接口  
**安全**：仅本地访问，动态端口

### 2. 代理配置

```rust
// 全局代理同步
tokio::spawn(async {
    sync_global_proxy_from_settings().await;
});
```

支持 HTTP/HTTPS/SOCKS5 代理

---

## 任务调度架构

### 唤醒任务系统 (Wakeup Scheduler)

```rust
// modules/wakeup_scheduler.rs
pub fn ensure_started(app_handle: AppHandle) {
    let mut scheduler = SCHEDULER.lock().unwrap();
    if scheduler.is_none() {
        *scheduler = Some(Scheduler::new(app_handle));
        restore_state_from_disk(); // 恢复持久化状态
    }
}

// Cron 表达式解析 + Tokio 定时任务
pub async fn schedule_wakeup_tasks(tasks: Vec<WakeupTask>) {
    for task in tasks {
        let cron = parse_cron(&task.schedule);
        tokio::spawn(run_cron_task(cron, task));
    }
}
```

### Codex 专属唤醒

```rust
// modules/codex_wakeup_scheduler.rs
// 支持 CLI 状态检测、会话可见性控制、线程同步
pub fn trigger_startup_tasks_if_needed(app_handle: AppHandle) {
    // 启动时检查是否需要立即执行唤醒
}
```

---

## 自动更新架构

### Tauri Updater 集成

```rust
// lib.rs
#[cfg(desktop)]
{
    app.handle()
        .plugin(tauri_plugin_updater::Builder::new().build())?;
    app.handle().plugin(tauri_plugin_process::init())?;
}
```

### 更新配置

```json
// tauri.conf.json
{
  "plugins": {
    "updater": {
      "pubkey": "...",
      "endpoints": [
        "https://github.com/jlcodes99/cockpit-tools/releases/latest/download/latest.json"
      ]
    }
  }
}
```

---

## 安全架构

### 1. 本地安全

- **数据存储**：全部本地，无云端同步
- **网络服务**：仅绑定 127.0.0.1
- **OAuth 流程**：标准 PKCE 或授权码模式
- **加密存储**：敏感 token 使用系统密钥链（如可用）

### 2. 应用安全

```rust
// lib.rs: 单实例锁
.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
    // 阻止多开，处理 deep link
}))

// CSP 配置 (开发时关闭)
"security": {
  "csp": null
}
```

### 3. 进程安全

```rust
// 实例进程监控
pub fn monitor_instance(pid: u32) {
    tokio::spawn(async move {
        let mut sys = System::new();
        loop {
            sys.refresh_processes();
            if sys.process(Pid::from(pid as usize)).is_none() {
                // 进程已退出，更新状态
                break;
            }
        }
    });
}
```

---

## 构建与部署

### CI/CD 工作流

| 工作流 | 功能 |
|-------|------|
| `build-matrix.yml` | 多平台构建矩阵 |
| `release.yml` | 发布流程 |
| `codeql.yml` | 安全扫描 |

### 构建脚本

```javascript
// package.json scripts
{
  "dev": "vite",
  "build": "npm run sync-version && tsc && vite build",
  "tauri": "npm run sync-version && tauri",
  "sync-version": "node scripts/sync-version.js"
}
```

### 发布平台

- **macOS**: `.dmg` (Apple Silicon + Intel)
- **Windows**: `.msi`, `.exe`
- **Linux**: `.deb`, `.AppImage`
- **Homebrew**: `jlcodes99/cockpit-tools`
- **Arch Linux**: AUR (`cockpit-tools` / `cockpit-tools-bin`)

---

## 性能优化

### 前端优化

1. **代码分割**：Vite 配置 manualChunks
```typescript
// vite.config.ts
manualChunks(id) {
  if (id.includes("node_modules")) {
    if (id.includes("/react/")) return "react-vendor";
    if (id.includes("/i18next/")) return "i18n-vendor";
    if (id.includes("/@tauri-apps/")) return "tauri-vendor";
  }
}
```

2. **懒加载**：所有页面组件异步加载
3. **状态持久化**：Zustand persist 中间件

### 后端优化

1. **异步架构**：Tokio 运行时
2. **后台线程**：托盘菜单初始化不阻塞主窗口
3. **缓存机制**：配额查询缓存

---

## 开发指南

### 前置要求

- Node.js v18+
- npm v9+
- Rust (Tauri 运行时)

### 开发命令

```bash
# 安装依赖
npm install

# 开发模式
npm run tauri dev

# 构建发布
npm run tauri build
```

### 项目统计

| 指标 | 数值 |
|-----|------|
| 前端 TS/TSX 文件 | 150+ |
| 后端 Rust 文件 | 120+ |
| 代码行数 (前端) | ~50,000 |
| 代码行数 (后端) | ~100,000 |
| 支持的 AI 平台 | 12 |
| 支持的语言 | 18 |
| IPC Commands | 300+ |

---

## 架构亮点

1. **模块化设计**：每个 AI 平台独立模块，便于扩展
2. **类型安全**：全栈 TypeScript + Rust 类型系统
3. **跨平台**：单一代码库支持 macOS/Windows/Linux
4. **高性能**：Rust 后端 + 异步 I/O
5. **可维护性**：清晰的职责分离，文档完善
6. **用户体验**：18 语言支持，实时配额监控，一键切换

---

## 扩展性设计

添加新 AI 平台需要：

1. **前端**：创建 `NewPlatformAccountsPage.tsx`
2. **后端**：
   - `modules/new_platform_account.rs`
   - `modules/new_platform_oauth.rs` (如需)
   - `modules/new_platform_instance.rs` (如需)
   - `commands/new_platform.rs`
   - `commands/new_platform_instance.rs`
3. **模型**：`models/new_platform.rs`
4. **前端 Store**：`stores/useNewPlatformAccountStore.ts`
5. **服务层**：`services/newPlatformService.ts`
6. **侧边栏**：`App.tsx` 添加导航项
7. **托盘**：`modules/tray.rs` 更新菜单

---

*文档生成时间: 2026-04-11*  
*基于 Cockpit Tools v0.21.1*
