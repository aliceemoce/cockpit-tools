import { invoke } from '@tauri-apps/api/core';

/**
 * 截取当前应用窗口截图
 * @param outputPath 可选输出路径
 * @returns 截图保存的文件路径
 */
export async function takeScreenshot(outputPath?: string): Promise<string> {
  return invoke<string>('take_screenshot', { outputPath: outputPath ?? null });
}

/**
 * 截取指定标题的窗口截图
 * @param windowTitle 窗口标题关键字
 * @param outputPath 可选输出路径
 * @returns 截图保存的文件路径
 */
export async function takeWindowScreenshot(
  windowTitle: string,
  outputPath?: string,
): Promise<string> {
  return invoke<string>('take_window_screenshot', {
    windowTitle,
    outputPath: outputPath ?? null,
  });
}

/**
 * 获取当前 UI 状态 JSON
 */
export async function getUiState(): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>('get_ui_state');
}

/**
 * 导出 UI 状态到文件
 * @param outputPath 可选输出路径
 * @returns 状态文件保存路径
 */
export async function exportUiState(outputPath?: string): Promise<string> {
  return invoke<string>('export_ui_state', { outputPath: outputPath ?? null });
}
