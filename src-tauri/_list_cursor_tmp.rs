use antigravity_cockpit_tools_lib::modules::{config, cursor_account, logger};
fn main() {
    logger::init_logger();
    let _ = config::get_user_config();
    let accounts = cursor_account::list_accounts();
    let hakee: Vec<_> = accounts.iter().filter(|a| a.email.to_lowercase().contains("hakee")).collect();
    println!("list_accounts_total={}", accounts.len());
    for a in &hakee { println!("hit id={} email={}", a.id, a.email); }
}
