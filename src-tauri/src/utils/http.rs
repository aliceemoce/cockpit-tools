use reqwest::Client;

/// 创建统一配置的 HTTP 客户端
pub fn create_client(timeout_secs: u64) -> Client {
    let mut builder = Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs));

    let config = crate::modules::config::get_user_config();
    if config.global_proxy_enabled && !config.global_proxy_url.trim().is_empty() {
        let proxy_url = config.global_proxy_url.trim();
        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
        }
    }

    builder.build().unwrap_or_else(|_| Client::new())
}
