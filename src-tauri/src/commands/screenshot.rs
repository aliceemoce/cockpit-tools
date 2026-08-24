//! WebView 截图命令模块
//!
//! 使用 PrintWindow + PW_RENDERFULLCONTENT 截取 WebView2 渲染内容。

use tauri::{AppHandle, Manager};

// 手动声明不在 windows crate 中的 Win32 函数
extern "system" {
    fn PrintWindow(hwnd: isize, hdcBlt: isize, nFlags: u32) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut RECT_OUT) -> i32;
}

#[repr(C)]
struct RECT_OUT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// 截取主窗口当前 WebView 画面，保存为 PNG 文件。
#[tauri::command]
pub fn take_screenshot(app: AppHandle, output_path: Option<String>) -> Result<String, String> {
    capture_main_window_screenshot(&app, output_path)
}

/// Deep link / 内部调用：PrintWindow 截主窗口 WebView（不用 xcap 裁屏）。
pub fn capture_main_window_screenshot(
    app: &AppHandle,
    output_path: Option<String>,
) -> Result<String, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;

    capture_window_content(&window, output_path)
}

/// 截取指定标签窗口的 WebView 画面
#[tauri::command]
pub fn take_window_screenshot(
    app: AppHandle,
    window_label: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let window = app
        .get_webview_window(&window_label)
        .ok_or_else(|| format!("未找到窗口: {}", window_label))?;

    capture_window_content(&window, output_path)
}

/// 截取窗口内容
fn capture_window_content(
    window: &tauri::WebviewWindow,
    output_path: Option<String>,
) -> Result<String, String> {
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("获取窗口句柄失败: {}", e))?;

    let raw_hwnd = hwnd.0 as isize;

    crate::modules::logger::log_info(&format!(
        "[Screenshot] 窗口 HWND: 0x{:x}",
        raw_hwnd as usize
    ));

    let png_data = capture_with_printwindow(raw_hwnd)?;
    save_png(png_data, output_path)
}

/// 使用 PrintWindow + PW_RENDERFULLCONTENT 截取窗口内容
#[cfg(target_os = "windows")]
fn capture_with_printwindow(hwnd: isize) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;

    unsafe {
        // 获取窗口大小
        let mut rect = RECT_OUT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return Err("获取窗口大小失败".to_string());
        }

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Err("窗口大小为 0".to_string());
        }

        crate::modules::logger::log_info(&format!(
            "[Screenshot] 窗口尺寸: {}x{}",
            width, height
        ));

        // 创建 DC 和位图
        let hdc_screen = GetDC(HWND::default());
        if hdc_screen.is_invalid() {
            return Err("获取屏幕 DC 失败".to_string());
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem.is_invalid() {
            let _ = ReleaseDC(HWND::default(), hdc_screen);
            return Err("创建兼容 DC 失败".to_string());
        }

        let hbmp = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);
        if hbmp.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(HWND::default(), hdc_screen);
            return Err("创建兼容位图失败".to_string());
        }

        let old_bmp = SelectObject(hdc_mem, hbmp);

        // PrintWindow with PW_RENDERFULLCONTENT (flag = 2)
        let pw_renderfullcontent: u32 = 0x00000002;
        let result = PrintWindow(hwnd, hdc_mem.0 as isize, pw_renderfullcontent);

        if result == 0 {
            let err = GetLastError();
            SelectObject(hdc_mem, old_bmp);
            let _ = DeleteObject(hbmp);
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(HWND::default(), hdc_screen);
            return Err(format!("PrintWindow 失败, error: {}", err.0));
        }

        // 从 HBITMAP 提取 PNG 数据
        let png_data = hbitmap_to_png(hbmp, hdc_mem, width, height)?;

        // 清理
        SelectObject(hdc_mem, old_bmp);
        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(HWND::default(), hdc_screen);

        Ok(png_data)
    }
}

/// 将 HBITMAP 转换为 PNG 数据
#[cfg(target_os = "windows")]
fn hbitmap_to_png(
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
    hdc_mem: windows::Win32::Graphics::Gdi::HDC,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    use windows::Win32::Graphics::Gdi::*;

    unsafe {
        let mut bmp_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // 自上而下
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let pixel_count = (width * height) as usize;
        let mut pixels = vec![0u8; pixel_count * 4];

        let result = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut bmp_info,
            DIB_RGB_COLORS,
        );

        if result == 0 {
            return Err("GetDIBits 失败".to_string());
        }

        // BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        // 编码为 PNG
        let img = image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| "创建图像失败".to_string())?;

        let mut png_data = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut png_data);
            let encoder = image::codecs::png::PngEncoder::new(&mut cursor);
            image::ImageEncoder::write_image(
                encoder,
                img.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("PNG 编码失败: {}", e))?;
        }

        Ok(png_data)
    }
}

/// 保存 PNG 数据到文件
fn save_png(data: Vec<u8>, output_path: Option<String>) -> Result<String, String> {
    if data.is_empty() {
        return Err("截图数据为空".to_string());
    }

    let path = output_path.unwrap_or_else(|| {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        format!(
            "{}/cockpit-screenshot-{}.png",
            std::env::temp_dir().display(),
            timestamp
        )
    });

    std::fs::write(&path, &data).map_err(|e| format!("保存截图失败: {}", e))?;

    crate::modules::logger::log_info(&format!(
        "[Screenshot] 截图已保存: {} ({} bytes)",
        path,
        data.len()
    ));

    Ok(path)
}
