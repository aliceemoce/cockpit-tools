from pathlib import Path
p = Path("src-tauri/src/commands/cursor_instance.rs")
text = p.read_text(encoding="utf-8")
pick_fn = """
fn pick_account_for_auto_switch(exclude: &HashSet<String>) -> Result<(String, &'static str, i32), String> {
    if let Some(full_id) = modules::cursor_account::pick_full_quota_account(exclude) {
        return Ok((full_id, \"full\", 1));
    }
    let id = modules::cursor_account::pick_highest_remaining_credits_account(exclude).ok_or_else(|| {
        \"没有可用的满额 Cursor 账号，请稍后重试或手动绑定账号\".to_string()
    })?;
    Ok((id, \"highest\", 1))
}

"""
if "fn pick_account_for_auto_switch" not in text:
    text = text.replace(
        "async fn resolve_auto_switch_account_with_probe_retry(",
        pick_fn + "async fn resolve_auto_switch_account_with_probe_retry(",
    )
start = text.index("        let pick = match modules::cursor_account::pick_cursor_rotation_account")
end = text.index("        let mut probe_ok = false;")
replacement = """        let (pick_id, pick_pool, pick_candidates) = match pick_account_for_auto_switch(&exclude) {
            Ok(value) => value,
            Err(err) => {
                modules::cursor_switch_audit::write_pick_no_candidates(trace, &err);
                return Err(err);
            }
        };
        let account = modules::cursor_account::load_account(&pick_id)
            .ok_or_else(|| format!(\"Cursor 账号不存在: {}\", pick_id))?;
        modules::cursor_switch_audit::write_pick(trace, &account, pick_pool, pick_candidates);

"""
text = text[:start] + replacement + text[end:]
text = text.replace(
    "probe_cursor_account_live_auth(&pick.account_id)",
    "probe_cursor_account_live_auth(&pick_id)",
)
text = text.replace("return Ok(pick.account_id);", "return Ok(pick_id);")
text = text.replace("exclude.insert(pick.account_id);", "exclude.insert(pick_id.clone());")
text = text.replace(
    "pick.pool,\n                modules::cursor_account::cursor_overview_remaining_percent(&account)\n                    .unwrap_or(pick.remaining_pct)",
    "pick_pool, 0",
)
idx = text.index("    let account_id = match forced_account_id {")
idx2 = text.index("        None => {", idx)
block = text[idx:idx2]
if "refresh_and_ensure_overview_pickable" in block:
    text = text.replace(
        block,
        """    let account_id = match forced_account_id {
        Some(id) => {
            if modules::cursor_account::load_account(&id).is_none() {
                return Err(format!(\"Cursor 账号不存在: {}\", id));
            }
            id
        }
""",
    )
p.write_text(text, encoding="utf-8")
print("ok ci")
