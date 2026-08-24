//! Cursor CDP 控制模块
//! 
//! 通过 Chrome DevTools Protocol 连接 Cursor 实例，精确操控 DOM 元素。
//! 用于程序化控制聊天、读取响应等自动化操作。

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

const CDP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CDP_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

fn cdp_http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .timeout(CDP_CONNECT_TIMEOUT)
            .build()
            .expect("CDP HTTP client")
    })
}

/// CDP 端口注册表：user_data_dir -> cdp_port
fn cdp_port_registry() -> &'static Mutex<HashMap<String, u16>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, u16>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 注册 CDP 端口
pub fn register_cdp_port(user_data_dir: &str, port: u16) {
    if let Ok(mut registry) = cdp_port_registry().lock() {
        registry.insert(user_data_dir.to_string(), port);
        crate::modules::logger::log_info(&format!(
            "[Cursor CDP] 注册端口: dir={}, port={}",
            user_data_dir, port
        ));
    }
}

/// 获取 CDP 端口
pub fn get_cdp_port(user_data_dir: &str) -> Option<u16> {
    cdp_port_registry()
        .lock()
        .ok()
        .and_then(|registry| registry.get(user_data_dir).copied())
}

/// CDP 端口是否已有可连 target
pub async fn cdp_port_has_targets(port: u16) -> bool {
    match get_cdp_targets(port).await {
        Ok(targets) => !targets.is_empty(),
        Err(e) => {
            crate::modules::logger::log_warn(&format!(
                "[Cursor CDP] 探测 target 失败 port={}: {}",
                port, e
            ));
            false
        }
    }
}

/// CDP Target 信息（Chrome `/json` 使用 camelCase）
#[derive(Debug, Clone, Deserialize)]
pub struct CdpTarget {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    #[serde(rename = "type")]
    target_type: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
}

/// 连接到 CDP 并获取所有 targets
pub async fn get_cdp_targets(port: u16) -> Result<Vec<CdpTarget>, String> {
    let url = format!("http://127.0.0.1:{}/json", port);

    let response = cdp_http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("CDP 请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("CDP 返回非成功状态: {}", response.status()));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 CDP 响应失败: {}", e))?;
    let targets: Vec<CdpTarget> = serde_json::from_str(&body).map_err(|e| {
        format!(
            "解析 CDP targets 失败: {}; body={}",
            e,
            body.chars().take(240).collect::<String>()
        )
    })?;

    Ok(targets
        .into_iter()
        .filter(|t| t.web_socket_debugger_url.is_some())
        .collect())
}

/// 通过 CDP 执行 JavaScript
pub async fn execute_javascript(port: u16, js_code: &str) -> Result<Value, String> {
    let targets = get_cdp_targets(port).await?;
    
    // 优先 page 型主窗（vscode-file / Cursor Agents），避免误选 webview。
    let pages: Vec<_> = targets
        .into_iter()
        .filter(|t| t.target_type == "page" || t.url.starts_with("vscode-file:"))
        .collect();
    let target = pages
        .iter()
        .find(|t| {
            let u = t.url.to_lowercase();
            let title = t.title.to_lowercase();
            u.contains("cursor")
                || u.contains("workbench")
                || title.contains("cursor")
                || title.contains("agents")
        })
        .or_else(|| pages.first())
        .cloned()
        .ok_or("未找到 Cursor 主页面 target")?;

    let ws_url = target
        .web_socket_debugger_url
        .ok_or("缺少 webSocketDebuggerUrl")?;

    // 连接 WebSocket
    let (mut ws, _) = timeout(CDP_CONNECT_TIMEOUT, connect_async(&ws_url))
        .await
        .map_err(|_| "CDP WebSocket 连接超时")?
        .map_err(|e| format!("CDP WebSocket 连接失败: {}", e))?;
    
    // 发送 Runtime.evaluate 命令
    let command = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {
            "expression": js_code,
            "returnByValue": true,
            "awaitPromise": true
        }
    });
    
    let msg = Message::Text(command.to_string().into());
    timeout(CDP_COMMAND_TIMEOUT, ws.send(msg))
        .await
        .map_err(|_| "CDP 命令发送超时")?
        .map_err(|e| format!("CDP 命令发送失败: {}", e))?;
    
    // 接收响应
    while let Some(msg) = timeout(CDP_COMMAND_TIMEOUT, ws.next()).await.map_err(|_| "CDP 响应超时")? {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id") == Some(&json!(1)) {
                        if let Some(result) = value.get("result") {
                            if let Some(exception) = result.get("exceptionDetails") {
                                return Err(format!("JS 执行异常: {}", exception));
                            }
                            return Ok(result.get("result").cloned().unwrap_or(Value::Null));
                        }
                    }
                }
            }
            Ok(_) => continue,
            Err(e) => return Err(format!("CDP WebSocket 错误: {}", e)),
        }
    }
    
    Err("CDP 未收到响应".to_string())
}

/// 打开 Cursor 聊天面板
pub async fn open_chat_panel(port: u16) -> Result<(), String> {
    // 尝试通过 CDP 触发聊天面板打开
    // Cursor 的聊天面板通常有一个特定的 CSS 选择器或命令
    let js_code = r#"
        (async () => {
            // 尝试找到聊天按钮并点击
            const chatButton = document.querySelector('[data-testid="chat-button"], .chat-button, [aria-label*="Chat"]');
            if (chatButton) {
                chatButton.click();
                return { success: true, method: 'button_click' };
            }
            
            // 尝试使用键盘快捷键 Ctrl+L
            const event = new KeyboardEvent('keydown', {
                key: 'l',
                code: 'KeyL',
                ctrlKey: true,
                bubbles: true
            });
            document.dispatchEvent(event);
            
            return { success: true, method: 'keyboard_shortcut' };
        })()
    "#;
    
    execute_javascript(port, js_code).await?;
    Ok(())
}

/// 发送聊天消息
pub async fn send_chat_message(port: u16, message: &str) -> Result<(), String> {
    // 转义消息中的特殊字符
    let escaped_message = message.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    
    let js_code = format!(
        r#"
        (async () => {{
            // 等待聊天面板打开
            await new Promise(resolve => setTimeout(resolve, 1000));
            
            // 找到聊天输入框
            const input = document.querySelector('[data-testid="chat-input"], .chat-input, textarea[placeholder*="Message"], [contenteditable="true"]');
            if (!input) {{
                return {{ success: false, error: '未找到聊天输入框' }};
            }}
            
            // 设置输入框内容
            if (input.tagName === 'TEXTAREA') {{
                input.value = "{}";
                input.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }} else {{
                input.textContent = "{}";
                input.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }}
            
            // 发送消息（模拟 Enter 键）
            const enterEvent = new KeyboardEvent('keydown', {{
                key: 'Enter',
                code: 'Enter',
                bubbles: true
            }});
            input.dispatchEvent(enterEvent);
            
            return {{ success: true, message: '{}' }};
        }})()
        "#,
        escaped_message, escaped_message, escaped_message
    );
    
    execute_javascript(port, &js_code).await?;
    Ok(())
}

/// 读取最新的聊天响应
pub async fn read_chat_response(port: u16) -> Result<String, String> {
    let js_code = r#"
        (async () => {
            // 等待响应生成
            await new Promise(resolve => setTimeout(resolve, 3000));
            
            // 查找最新的 AI 响应
            const messages = document.querySelectorAll('[data-testid="message"], .message, [role="assistant"], .assistant-message');
            if (messages.length === 0) {
                return { success: false, error: '未找到聊天消息' };
            }
            
            // 获取最后一条消息
            const lastMessage = messages[messages.length - 1];
            const text = lastMessage.textContent || lastMessage.innerText;
            
            return { success: true, response: text.trim() };
        })()
    "#;
    
    let result = execute_javascript(port, js_code).await?;
    
    if let Some(obj) = result.as_object() {
        if obj.get("success") == Some(&json!(true)) {
            if let Some(response) = obj.get("response") {
                return Ok(response.as_str().unwrap_or("").to_string());
            }
        }
        if let Some(error) = obj.get("error") {
            return Err(format!("读取响应失败: {}", error.as_str().unwrap_or("未知错误")));
        }
    }
    
    Err("无法解析聊天响应".to_string())
}

/// 截图当前页面
pub async fn take_screenshot(port: u16) -> Result<Vec<u8>, String> {
    let targets = get_cdp_targets(port).await?;
    
    let target = targets
        .into_iter()
        .find(|t| t.url.contains("cursor") || t.title.contains("Cursor"))
        .ok_or("未找到 Cursor 主页面 target")?;

    let ws_url = target
        .web_socket_debugger_url
        .ok_or("缺少 webSocketDebuggerUrl")?;

    let (mut ws, _) = timeout(CDP_CONNECT_TIMEOUT, connect_async(&ws_url))
        .await
        .map_err(|_| "CDP WebSocket 连接超时")?
        .map_err(|e| format!("CDP WebSocket 连接失败: {}", e))?;
    
    // 发送 Page.captureScreenshot 命令
    let command = json!({
        "id": 1,
        "method": "Page.captureScreenshot",
        "params": {
            "format": "png"
        }
    });
    
    let msg = Message::Text(command.to_string().into());
    timeout(CDP_COMMAND_TIMEOUT, ws.send(msg))
        .await
        .map_err(|_| "CDP 截图命令发送超时")?
        .map_err(|e| format!("CDP 截图命令发送失败: {}", e))?;
    
    // 接收响应
    while let Some(msg) = timeout(CDP_COMMAND_TIMEOUT, ws.next()).await.map_err(|_| "CDP 截图响应超时")? {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id") == Some(&json!(1)) {
                        if let Some(result) = value.get("result") {
                            if let Some(data) = result.get("data").and_then(|d| d.as_str()) {
                                // Base64 解码
                                use base64::{Engine as _, engine::general_purpose::STANDARD};
                                let bytes = STANDARD.decode(data)
                                    .map_err(|e| format!("Base64 解码失败: {}", e))?;
                                return Ok(bytes);
                            }
                        }
                    }
                }
            }
            Ok(_) => continue,
            Err(e) => return Err(format!("CDP WebSocket 错误: {}", e)),
        }
    }
    
    Err("CDP 未收到截图响应".to_string())
}

/// 窗内 Agents 对话验活结果
pub struct ComposerDialogueVerify {
    pub dialogue_ok: bool,
    pub probe_tag: String,
    pub error: Option<String>,
    pub tail_excerpt: String,
}

/// 经 CDP 在 Cursor Agents 页发一条带唯一标记的消息，等待 AI 回复含预期答案。
pub async fn verify_agents_composer_dialogue(port: u16) -> Result<ComposerDialogueVerify, String> {
    let probe_tag = format!(
        "ckpt{}",
        chrono::Utc::now().format("%H%M%S")
    );
    let message = format!(
        "总控验活标记{}：一加一等于几？请只回复阿拉伯数字2。",
        probe_tag
    );
    let expect_answer = "2";

    crate::modules::logger::log_info(&format!(
        "[Cursor CDP] verify_agents_composer_dialogue 开始 port={} tag={}",
        port, probe_tag
    ));

    let mut client = CdpSession::connect(port).await?;
    client.call("Page.bringToFront", json!({})).await?;
    tokio::time::sleep(Duration::from_millis(400)).await;

    let dom = client
        .eval_value(
            r#"
        (() => {
          function walk(root, out) {
            const nodes = root.querySelectorAll ? root.querySelectorAll('*') : [];
            for (const n of nodes) { out.push(n); if (n.shadowRoot) walk(n.shadowRoot, out); }
          }
          const all=[]; walk(document, all);
          let input=null, send=null;
          for (const n of all) {
            const cls=(n.className&&String(n.className))||'';
            if (!input && n.isContentEditable && cls.includes('ProseMirror')) input=n;
            if (!send && cls.includes('ui-prompt-input-toolbar__right')) send=n;
          }
          const ir=input?input.getBoundingClientRect():null;
          const sr=send?send.getBoundingClientRect():null;
          return {
            hasInput: !!input,
            ix: ir?Math.round(ir.x+ir.width/2):null,
            iy: ir?Math.round(ir.y+ir.height/2):null,
            sx: sr?Math.round(sr.x+sr.width-12):null,
            sy: sr?Math.round(sr.y+sr.height/2):null,
          };
        })()
        "#,
        )
        .await?;

    let has_input = dom
        .get("hasInput")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !has_input {
        return Ok(ComposerDialogueVerify {
            dialogue_ok: false,
            probe_tag,
            error: Some("未找到 Agents 输入框".to_string()),
            tail_excerpt: String::new(),
        });
    }

    let ix = dom.get("ix").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
    let iy = dom.get("iy").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
    client.click(ix, iy).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let escaped = message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let set_js = format!(
        r#"
        (() => {{
          const input = document.querySelector('.tiptap.ProseMirror');
          if (!input) return {{ok:false, err:'no-input'}};
          input.focus();
          input.innerHTML = '<p>{}</p>';
          input.dispatchEvent(new InputEvent('input', {{ bubbles: true }}));
          return {{ok:true, text:(input.textContent||'').slice(0,120)}};
        }})()
        "#,
        escaped
    );
    let set_result = client.eval_value(&set_js).await?;
    if set_result.get("ok") != Some(&json!(true)) {
        return Ok(ComposerDialogueVerify {
            dialogue_ok: false,
            probe_tag,
            error: Some(format!("写入输入框失败: {:?}", set_result)),
            tail_excerpt: String::new(),
        });
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    let sx = dom.get("sx").and_then(|v| v.as_i64());
    let sy = dom.get("sy").and_then(|v| v.as_i64());
    if let (Some(sx), Some(sy)) = (sx, sy) {
        client.click(sx as f64, sy as f64).await?;
    } else {
        client
            .dispatch_key("Enter", "Enter", 13, Some("\r"))
            .await?;
    }

    let baseline = client
        .eval_value(
            r#"(() => ({ len: (document.body && document.body.innerText || '').length }))()"#,
        )
        .await
        .ok();

    let mut dialogue_ok = false;
    let mut tail_excerpt = String::new();
    let mut last_error: Option<String> = None;

    for _ in 0..24 {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let poll = client
            .eval_value(
                r#"
            (() => {
              const text = document.body ? (document.body.innerText || '') : '';
              return { len: text.length, tail: text.slice(-1600) };
            })()
            "#,
            )
            .await?;

        tail_excerpt = poll
            .get("tail")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let grew = match (&baseline, poll.get("len")) {
            (Some(base), Some(len)) => {
                let base_len = base.get("len").and_then(|v| v.as_i64()).unwrap_or(0);
                len.as_i64().unwrap_or(0) > base_len + 8
            }
            _ => true,
        };

        if grew
            && tail_excerpt.contains(&probe_tag)
            && tail_excerpt.contains(expect_answer)
        {
            // 标记后仍须出现独立回复区的新内容：标记与「2」之间应有助手回复结构
            let tag_pos = tail_excerpt.rfind(&probe_tag).unwrap_or(0);
            let after_tag = &tail_excerpt[tag_pos + probe_tag.len()..];
            if after_tag.contains(expect_answer) {
                dialogue_ok = true;
                break;
            }
        }
    }

    if !dialogue_ok && last_error.is_none() {
        last_error = Some("等待 AI 回复超时或未含预期答案".to_string());
    }

    let result = ComposerDialogueVerify {
        dialogue_ok,
        probe_tag: probe_tag.clone(),
        error: if dialogue_ok { None } else { last_error },
        tail_excerpt: tail_excerpt.chars().take(600).collect(),
    };

    crate::modules::logger::log_info(&format!(
        "[Cursor CDP] verify_agents_composer_dialogue 结束 port={} ok={} err={:?}",
        port, result.dialogue_ok, result.error
    ));

    Ok(result)
}

struct CdpSession {
    ws: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    rx: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    >,
    next_id: u32,
    port: u16,
}

impl CdpSession {
    async fn connect(port: u16) -> Result<Self, String> {
        let targets = get_cdp_targets(port).await?;
        let target = targets
            .iter()
            .find(|t| t.title.contains("Agents") || t.title.contains("Cursor"))
            .or_else(|| targets.first())
            .ok_or("未找到 CDP page target")?;

        let ws_url = target
            .web_socket_debugger_url
            .as_deref()
            .ok_or("缺少 webSocketDebuggerUrl")?;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::http::HeaderValue;

        let mut request = ws_url
            .into_client_request()
            .map_err(|e| format!("CDP WS 请求构建失败: {}", e))?;
        if let Ok(origin) = HeaderValue::from_str(&format!("http://127.0.0.1:{}", port)) {
            request.headers_mut().insert("Origin", origin);
        }

        let (ws, _) = timeout(CDP_CONNECT_TIMEOUT, connect_async(request))
            .await
            .map_err(|_| "CDP WebSocket 连接超时")?
            .map_err(|e| format!("CDP WebSocket 连接失败: {}", e))?;

        let (tx, rx) = ws.split();
        Ok(Self {
            ws: tx,
            rx,
            next_id: 0,
            port,
        })
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let command = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.ws
            .send(Message::Text(command.to_string().into()))
            .await
            .map_err(|e| format!("CDP 发送失败: {}", e))?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        let mut stray_events = 0u32;

        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let wait = remaining.min(CDP_COMMAND_TIMEOUT);
            let msg = match timeout(wait, self.rx.next()).await {
                Ok(Some(Ok(m))) => m,
                Ok(Some(Err(e))) => return Err(format!("CDP WebSocket 错误: {}", e)),
                Ok(None) => break,
                Err(_) => break,
            };

            match msg {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        if value.get("id") == Some(&json!(id)) {
                            if let Some(err) = value.get("error") {
                                return Err(format!("CDP 错误: {}", err));
                            }
                            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                        }
                    }
                    stray_events += 1;
                    if stray_events > 500 {
                        return Err("CDP 杂讯事件过多，未收到命令响应".to_string());
                    }
                }
                _ => continue,
            }
        }
        Err("CDP 未收到响应".to_string())
    }

    async fn eval_value(&mut self, expression: &str) -> Result<Value, String> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("JS 执行异常: {}", exception));
        }
        Ok(result
            .get("result")
            .and_then(|inner| inner.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn click(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1,
            }),
        )
        .await?;
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1,
            }),
        )
        .await?;
        Ok(())
    }

    async fn dispatch_key(
        &mut self,
        key: &str,
        code: &str,
        vk: u32,
        text: Option<&str>,
    ) -> Result<(), String> {
        let mut down = json!({
            "type": "keyDown",
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": vk,
            "nativeVirtualKeyCode": vk,
        });
        if let Some(t) = text {
            down["text"] = json!(t);
        }
        self.call("Input.dispatchKeyEvent", down).await?;
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": vk,
                "nativeVirtualKeyCode": vk,
            }),
        )
        .await?;
        Ok(())
    }

    #[allow(dead_code)]
    fn port(&self) -> u16 {
        self.port
    }
}
