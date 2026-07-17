// Windows：始终用 GUI 子系统，避免 debug 构建弹出滚动的控制台窗口（日志走 app.log）。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    antigravity_cockpit_tools_lib::run()
}
