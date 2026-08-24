use clap::{Parser, Subcommand};
use cockpit_core::modules::{
    codebuddy_account, codebuddy_cn_account, codex_account, cursor_account, github_copilot_account,
    kiro_account, qoder_account, trae_account, windsurf_account, workbuddy_account, zed_account,
};
use colored::*;
use tabled::{Table, Tabled};

#[derive(Parser)]
#[command(author, version, about = "Cockpit Tools CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 列出指定平台的账号
    List {
        /// 平台名（cursor, codex, copilot, windsurf, kiro, codebuddy, codebuddy_cn, qoder, trae, zed, zcode, workbuddy）
        platform: String,
    },
    /// 切换账号（注入到目标 IDE）
    Switch {
        /// 平台名
        platform: String,
        /// 账号 ID 或邮箱
        account: String,
    },
    /// 显示当前活跃账号
    Current {
        /// 平台名
        platform: String,
    },
    /// 对话验活（Cursor: 用 Agent CLI 发最小对话验证）
    Probe {
        /// 平台名（目前仅支持 cursor）
        platform: String,
        /// 账号 ID
        account_id: String,
    },
    /// 显示账号配额信息
    Quota {
        /// 平台名
        platform: String,
        /// 账号 ID（可选，不指定则显示当前账号）
        account_id: Option<String>,
    },
    /// 截图（需要 GUI 正在运行，通过 WebSocket 连接）
    Screenshot {
        /// 输出路径（可选）
        output_path: Option<String>,
    },
    /// 获取 UI 状态 JSON
    UiState,
    /// 显示所有支持的平台列表
    Platforms,
}

/// 通用账号显示结构
#[derive(Tabled)]
struct AccountDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Email")]
    email: String,
    #[tabled(rename = "Plan")]
    plan: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

/// 平台列表
const SUPPORTED_PLATFORMS: &[&str] = &[
    "cursor",
    "codex",
    "copilot",
    "windsurf",
    "kiro",
    "codebuddy",
    "codebuddy_cn",
    "qoder",
    "trae",
    "zed",
    "workbuddy",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List { platform }) => list_accounts(&platform),
        Some(Commands::Switch { platform, account }) => switch_account(&platform, &account),
        Some(Commands::Current { platform }) => show_current(&platform),
        Some(Commands::Probe {
            platform,
            account_id,
        }) => probe_account(&platform, &account_id),
        Some(Commands::Quota {
            platform,
            account_id,
        }) => show_quota(&platform, account_id.as_deref()),
        Some(Commands::Screenshot { output_path }) => take_screenshot(output_path),
        Some(Commands::UiState) => show_ui_state(),
        Some(Commands::Platforms) => show_platforms(),
        None => {
            println!("Cockpit Tools CLI v{}", env!("CARGO_PKG_VERSION"));
            println!("用法: cockpit <命令> <平台> [参数]");
            println!("运行 cockpit --help 查看完整命令列表");
            println!("\n支持的平台: {}", SUPPORTED_PLATFORMS.join(", "));
        }
    }

    Ok(())
}

fn normalize_platform(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "github_copilot" | "github-copilot" | "copilot" => "copilot".to_string(),
        "codebuddy_cn" | "codebuddy-cn" => "codebuddy_cn".to_string(),
        other => other.to_string(),
    }
}

fn list_accounts(platform: &str) {
    let platform = normalize_platform(platform);
    let accounts: Vec<AccountDisplay> = match platform.as_str() {
        "cursor" => cursor_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.membership_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "codex" => codex_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "copilot" => github_copilot_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.github_email.unwrap_or_default(),
                plan: a.copilot_plan.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "windsurf" => windsurf_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.github_login,
                plan: a.copilot_plan.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "kiro" => kiro_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_tier.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "codebuddy" => codebuddy_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "codebuddy_cn" => codebuddy_cn_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "qoder" => qoder_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "trae" => trae_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "zed" => zed_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.github_login.clone(),
                plan: a.plan_raw.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        "workbuddy" => workbuddy_account::list_accounts()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                plan: a.plan_type.unwrap_or_default(),
                tags: a.tags.map(|t| t.join(", ")).unwrap_or_default(),
            })
            .collect(),
        _ => {
            println!("{} 未知平台: {}", "Error:".red(), platform);
            println!("支持的平台: {}", SUPPORTED_PLATFORMS.join(", "));
            return;
        }
    };

    if accounts.is_empty() {
        println!("平台 '{}' 暂无账号", platform);
    } else {
        println!("平台 '{}' 共 {} 个账号:", platform, accounts.len());
        println!("{}", Table::new(&accounts).to_string());
    }
}

fn switch_account(platform: &str, account: &str) {
    let platform = normalize_platform(platform);
    let result = match platform.as_str() {
        "cursor" => cursor_account::inject_to_cursor(account),
        "qoder" => qoder_account::inject_to_qoder(account),
        "trae" => trae_account::inject_to_trae(account),
        _ => {
            println!(
                "{} 平台 '{}' 的 switch 暂未实现（仅 cursor/qoder/trae 支持）",
                "Info:".yellow(),
                platform
            );
            return;
        }
    };

    match result {
        Ok(_) => println!(
            "{} 已切换 {} 账号到 {}",
            "Success:".green(),
            platform,
            account
        ),
        Err(e) => println!("{} {}", "Error:".red(), e),
    }
}

fn show_current(platform: &str) {
    let platform = normalize_platform(platform);
    // 列出所有账号，标记当前（简单实现：显示第一个或全部）
    println!("平台 '{}' 的当前活跃账号:", platform);
    list_accounts(&platform);
}

fn probe_account(platform: &str, account_id: &str) {
    let platform = normalize_platform(platform);
    match platform.as_str() {
        "cursor" => {
            println!("正在对账号 {} 执行对话验活...", account_id);
            println!("（使用 Cursor Agent CLI 发送最小对话请求）");

            // 读取账号信息
            let account = match cursor_account::load_account(account_id) {
                Some(a) => a,
                None => {
                    // 尝试按邮箱查找
                    let accounts = cursor_account::list_accounts();
                    match accounts.into_iter().find(|a| a.email == account_id) {
                        Some(a) => a,
                        None => {
                            println!("{} 账号 '{}' 不存在", "Error:".red(), account_id);
                            return;
                        }
                    }
                }
            };

            if account.access_token.trim().is_empty() {
                println!("{} 账号缺少 access_token", "Error:".red());
                return;
            }

            println!("账号: {}", account.email);
            println!("正在验活...");

            // 注意：完整的 chat_probe 需要 cursor_chat_probe 模块（在 src-tauri 中）
            // CLI 版本仅做基本检查（token 是否存在）
            println!(
                "{} Token 存在，长度 {} 字符",
                "Info:".cyan(),
                account.access_token.len()
            );
            println!(
                "{} 完整对话验活请通过 GUI 或 deep link (cockpit-tools://probe/cursor/{})",
                "Info:".yellow(),
                account.id
            );
        }
        _ => {
            println!(
                "{} 平台 '{}' 的 probe 暂未实现",
                "Info:".yellow(),
                platform
            );
        }
    }
}

fn show_quota(platform: &str, account_id: Option<&str>) {
    let platform = normalize_platform(platform);
    println!("平台 '{}' 的配额信息:", platform);
    if let Some(id) = account_id {
        println!("  账号: {}", id);
    }
    println!(
        "{} 配额查询需要通过 GUI 或 WebSocket 接口获取",
        "Info:".yellow()
    );
    println!("提示: 运行 Cockpit Tools GUI 后，使用 cockpit-tools://ui-state 获取状态");
}

fn take_screenshot(output_path: Option<String>) {
    println!("截图功能需要通过运行中的 Cockpit Tools GUI 获取。");
    println!("请确保 Cockpit Tools 正在运行，然后使用:");
    println!("  cockpit-tools://screenshot?path=<输出路径>");
    println!("或在前端界面点击截图按钮。");
    if let Some(path) = output_path {
        println!("\n期望输出路径: {}", path);
    }
}

fn show_ui_state() {
    println!("UI 状态获取需要通过运行中的 Cockpit Tools GUI。");
    println!("请确保 Cockpit Tools 正在运行，然后使用:");
    println!("  cockpit-tools://ui-state");
    println!("\n状态文件将保存在: {}/cockpit-ui-state.json", std::env::temp_dir().display());
}

fn show_platforms() {
    println!("Cockpit Tools 支持的平台:\n");
    for p in SUPPORTED_PLATFORMS {
        let switch_support = match *p {
            "cursor" | "qoder" | "trae" => "✓",
            _ => "—",
        };
        println!("  {:15} 切号: {}", p, switch_support);
    }
}
