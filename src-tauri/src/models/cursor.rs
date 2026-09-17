use serde::{Deserialize, Serialize};

/// Cursor Agent CLI 真实对话验活结果（与 usage-summary 百分比独立）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorChatProbe {
    /// ok | rate_limited | auth_failed | network_error | unknown_error | agent_missing
    pub outcome: String,
    pub probed_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorAccount {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_up_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_auth_raw: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_usage_raw: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_query_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_query_last_error_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_updated_at: Option<i64>,

    /// 真实 Agent 对话验活；未验活时为空，不得用 usage-summary 冒充。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_probe: Option<CursorChatProbe>,

    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorAccountSummary {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorAccountIndex {
    pub version: String,
    pub accounts: Vec<CursorAccountSummary>,
}

impl CursorAccountIndex {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            accounts: Vec::new(),
        }
    }
}

impl Default for CursorAccountIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorImportPayload {
    pub email: String,
    pub auth_id: Option<String>,
    pub name: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub membership_type: Option<String>,
    pub subscription_status: Option<String>,
    pub sign_up_type: Option<String>,
    pub cursor_auth_raw: Option<serde_json::Value>,
    pub cursor_usage_raw: Option<serde_json::Value>,
    pub status: Option<String>,
    pub status_reason: Option<String>,
}

/// 前端列表分页：不含 token / auth_raw，避免四千号一次 IPC 堵死整窗。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorAccountListPage {
    pub accounts: Vec<CursorAccount>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: usize,
    pub has_more: bool,
}

/// 0012：当前账号**实时**额度快照。
/// `queried=false` 表示这次没真正拉到 usage（网络/401/未绑定），
/// 前端**不得**用磁盘旧值充当满额度，必须显示「未刷新/查询失败」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorCurrentQuotaSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub queried: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_percent: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<CursorAccount>,
}

impl CursorAccount {
    pub fn summary(&self) -> CursorAccountSummary {
        CursorAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            auth_id: self.auth_id.clone(),
            tags: self.tags.clone(),
            membership_type: self.membership_type.clone(),
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }

    /// 列表 UI 用：去掉令牌与 auth 原文；保留 usage_raw 供额度展示。
    pub fn for_ui_list(mut self) -> Self {
        self.access_token.clear();
        self.refresh_token = None;
        self.cursor_auth_raw = None;
        self
    }
}
