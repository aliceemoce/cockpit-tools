/**
 * Windsurf 自动注册 Node.js 脚本
 * 使用 Playwright 实现 Windsurf OAuth 自动授权
 * 
 * 参数: node auto_register_windsurf.js [proxyUrl]
 */

const { chromium } = require('playwright');
const fs = require('fs');
const { execSync } = require('child_process');
const http = require('http');
const url = require('url');

const WINDSURF_AUTH_URL = 'https://www.windsurf.com/windsurf/signin';
const WINDSURF_CLIENT_ID = '3GUryQ7ldAeKEuD2obYnppsnmj58eP5u';
const CALLBACK_PORT = 0; // 随机端口

// 日志输出到 stderr（这样 stdout 可以保留给 JSON 结果）
function log(message) {
  console.error(`[LOG] ${message}`);
}

// ==================== 浏览器检测和安装 ====================

function checkChromiumInstalled() {
  try {
    const chromiumPath = chromium.executablePath();
    if (fs.existsSync(chromiumPath)) {
      log(`找到 Chromium: ${chromiumPath}`);
      return true;
    }
  } catch (e) {}
  return false;
}

function installChromium() {
  try {
    log('正在安装 Chromium...');
    execSync('npx playwright install chromium', { 
      stdio: 'inherit',
      timeout: 300000
    });
    log('Chromium 安装完成');
    return true;
  } catch (e) {
    log(`安装 Chromium 失败: ${e.message}`);
    return false;
  }
}

function findSystemChrome() {
  const possiblePaths = [
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    process.env.LOCALAPPDATA + '\\Google\\Chrome\\Application\\chrome.exe',
    process.env.PROGRAMFILES + '\\Google\\Chrome\\Application\\chrome.exe',
    process.env['PROGRAMFILES(X86)'] + '\\Google\\Chrome\\Application\\chrome.exe',
  ];
  
  for (const chromePath of possiblePaths) {
    if (chromePath && fs.existsSync(chromePath)) {
      log(`找到系统 Chrome: ${chromePath}`);
      return chromePath;
    }
  }
  return null;
}

function findSystemEdge() {
  const possiblePaths = [
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    process.env.LOCALAPPDATA + '\\Microsoft\\Edge\\Application\\msedge.exe',
    process.env.PROGRAMFILES + '\\Microsoft\\Edge\\Application\\msedge.exe',
    process.env['PROGRAMFILES(X86)'] + '\\Microsoft\\Edge\\Application\\msedge.exe',
  ];
  
  for (const edgePath of possiblePaths) {
    if (edgePath && fs.existsSync(edgePath)) {
      log(`找到系统 Edge: ${edgePath}`);
      return edgePath;
    }
  }
  return null;
}

// 随机延迟（模拟真人思考时间）
function randomDelay(min, max) {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

// 像真人一样逐字符输入，带随机延迟
async function humanLikeFill(page, selector, value, description) {
  log(`正在输入${description}...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout: 10000 });
    
    // 先点击获取焦点
    await element.click();
    await page.waitForTimeout(randomDelay(100, 300));
    
    // 清除现有内容（模拟真人按 Ctrl+A 然后 Delete）
    await element.press('Control+a');
    await page.waitForTimeout(randomDelay(50, 150));
    await element.press('Delete');
    await page.waitForTimeout(randomDelay(100, 200));
    
    // 逐字符输入，带随机延迟
    for (const char of value) {
      await element.press(char);
      // 输入延迟：50-200ms，偶尔更慢模拟思考
      const delay = Math.random() > 0.9 ? randomDelay(200, 500) : randomDelay(50, 150);
      await page.waitForTimeout(delay);
    }
    
    log(`✓ 已输入${description}`);
    return true;
  } catch (error) {
    log(`✗ ${description}输入失败: ${error.message}`);
    return false;
  }
}

// ==================== 页面操作辅助函数 ====================

async function waitAndFill(page, selector, value, description, timeout = 30000) {
  log(`等待${description}出现...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout });
    await page.waitForTimeout(500);
    await element.clear();
    await element.fill(value);
    log(`✓ 已输入${description}`);
    return true;
  } catch (error) {
    log(`✗ ${description}操作失败: ${error.message}`);
    return false;
  }
}

// 等待用户手动点击按钮（检测按钮消失、页面跳转或进入下一步）
async function waitForUserClick(page, buttonSelector, description, maxWaitMs = 90000, expectedNextStep = null) {
  log(`⏳ 请手动点击【${description}】（最长等待${maxWaitMs/1000}秒）...`);
  
  const fastCheckInterval = 500; // 快速检测间隔 500ms
  const logInterval = 5000; // 每5秒输出日志
  let lastUrl = page.url();
  let initialStep = null;
  let lastLogTime = 0;
  
  const startTime = Date.now();
  
  // 获取初始步骤
  try {
    initialStep = await detectCurrentStep(page);
  } catch (e) {}
  
  while (Date.now() - startTime < maxWaitMs) {
    const currentUrl = page.url();
    const remainingSeconds = Math.floor((maxWaitMs - (Date.now() - startTime)) / 1000);
    const elapsed = Date.now() - startTime;
    
    // 检测1：页面 URL 发生变化
    if (currentUrl !== lastUrl) {
      log(`✓ 检测到页面跳转 (${currentUrl})`);
      await page.waitForTimeout(randomDelay(500, 1000));
      return { success: true, navigated: true };
    }
    
    // 检测2：按钮消失（被点击后）
    const buttonVisible = await page.locator(buttonSelector).isVisible().catch(() => false);
    if (!buttonVisible) {
      // 按钮不见了，可能是被点击了，稍等确认
      await page.waitForTimeout(300);
      const stillNotVisible = await page.locator(buttonSelector).isVisible().catch(() => false);
      if (!stillNotVisible) {
        log(`✓ 检测到【${description}】已被点击（按钮消失）`);
        await page.waitForTimeout(randomDelay(800, 1500));
        return { success: true, navigated: false };
      }
    }
    
    // 检测3：步骤发生变化（进入下一步）
    try {
      const currentStep = await detectCurrentStep(page);
      if (currentStep && currentStep !== initialStep) {
        log(`✓ 检测到步骤变化: ${initialStep} → ${currentStep}`);
        await page.waitForTimeout(randomDelay(800, 1500));
        return { success: true, navigated: true };
      }
    } catch (e) {}
    
    // 每5秒输出一次日志
    if (elapsed - lastLogTime >= logInterval) {
      const step = await detectCurrentStep(page);
      log(`📍 当前步骤: ${step || '等待中'} | 剩余${remainingSeconds}秒 | 请手动点击【${description}】`);
      lastLogTime = elapsed;
    }
    
    await page.waitForTimeout(fastCheckInterval);
  }
  
  log(`✗ 等待【${description}】点击超时`);
  return { success: false, error: '等待用户点击超时' };
}

// 检测当前页面处于哪一步
async function detectCurrentStep(page) {
  try {
    const url = page.url();
    
    // 根据页面特征检测当前步骤（按优先级顺序）
    
    // 1. 授权回调完成
    if (url.includes('windsurf-auth-callback') || url.startsWith('http://127.0.0.1')) {
      return '授权回调页（已完成）';
    }
    
    // 2. 已登录到编辑器
    if (url.includes('windsurf.com/editor') || url.includes('/ide/')) {
      const hasEditor = await page.locator('text=Windsurf, .windsurf-editor, .monaco-editor, [class*="editor"]').isVisible().catch(() => false);
      if (hasEditor) return 'Windsurf编辑器（已登录）';
    }
    
    // 3. 验证完成页（绿色勾）
    const hasSuccessMark = await page.locator('.success-checkmark, .check-icon, [class*="success"], text=成功, text=Success, text=✓').isVisible().catch(() => false);
    if (hasSuccessMark) return '验证完成页（需点击Continue）';
    
    // 4. 人机验证页
    const hasCaptcha = await page.locator('text=verify that you are human, .cf-turnstile, iframe[title*="challenge"], .captcha').isVisible().catch(() => false);
    if (hasCaptcha) return '人机验证页';
    
    // 5. 设置密码页（优先检测，因为有密码确认字段）
    const hasConfirmPassword = await page.locator('input[name*="confirm"], input[placeholder*="confirm" i], input[placeholder*="确认" i]').isVisible().catch(() => false);
    const hasPasswordWithConfirm = await page.locator('input[type="password"]').count().catch(() => 0) >= 2;
    if (hasConfirmPassword || hasPasswordWithConfirm) return '设置密码页';
    
    // 6. 登录页（Sign up 链接）
    const hasSignUpLink = await page.locator('a[href*="signup"], a:has-text("Sign up"), a:has-text("sign up")').isVisible().catch(() => false);
    if (hasSignUpLink) return '登录页（需要点击Sign up）';
    
    // 7. 填写姓名/邮箱页（有First name和Last name字段）
    const hasFirstName = await page.locator('input[name="firstName"], input[placeholder*="first name" i]').isVisible().catch(() => false);
    const hasLastName = await page.locator('input[name="lastName"], input[placeholder*="last name" i]').isVisible().catch(() => false);
    if (hasFirstName || hasLastName) return '填写姓名/邮箱页';
    
    // 8. 简单的密码页（只有一个密码字段）
    const hasSinglePassword = await page.locator('input[type="password"]').isVisible().catch(() => false);
    if (hasSinglePassword) return '设置密码页';
    
    // 9. 检测是否有Continue按钮但没有其他特征
    const hasContinueButton = await page.locator('button:has-text("Continue"), button[type="submit"]').isVisible().catch(() => false);
    if (hasContinueButton) return '表单页（需点击Continue）';
    
    return null;
  } catch (e) {
    return null;
  }
}

async function waitAndClick(page, selector, description, timeout = 30000) {
  log(`等待${description}出现...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout });
    await page.waitForTimeout(500);
    await element.click();
    log(`✓ 已点击${description}`);
    return true;
  } catch (error) {
    log(`✗ ${description}点击失败: ${error.message}`);
    return false;
  }
}

// ==================== 回调服务器 ====================

function startCallbackServer() {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      const parsedUrl = url.parse(req.url, true);
      
      if (parsedUrl.pathname === '/windsurf-auth-callback') {
        const query = parsedUrl.query;
        log(`收到回调: ${JSON.stringify(query)}`);
        
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end('<html><body><h1>授权成功！可以关闭此页面。</h1></body></html>');
        
        server.close();
        
        if (query.access_token) {
          resolve({
            success: true,
            accessToken: query.access_token,
            tokenType: query.token_type || 'Bearer',
            expiresIn: query.expires_in,
            state: query.state
          });
        } else if (query.error) {
          resolve({
            success: false,
            error: query.error_description || query.error
          });
        } else {
          resolve({
            success: false,
            error: '未获取到 access_token'
          });
        }
      } else {
        res.writeHead(404);
        res.end('Not Found');
      }
    });
    
    server.listen(0, '127.0.0.1', () => {
      const port = server.address().port;
      log(`回调服务器启动: http://127.0.0.1:${port}/windsurf-auth-callback`);
      resolve({ port, server });
    });
    
    server.on('error', (err) => {
      reject(err);
    });
  });
}

// ==================== 表单填写 ====================

function generateRandomName() {
  const names = ['Alex', 'Jordan', 'Taylor', 'Casey', 'Morgan', 'Riley', 'Quinn', 'Avery', 'Skyler', 'Dakota', 'Reese', 'Rowan', 'Sage', 'Phoenix', 'Eden'];
  return names[Math.floor(Math.random() * names.length)];
}

function generateRandomEmail() {
  const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
  let local = '';
  for (let i = 0; i < 10; i++) {
    local += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return `${local}@gmail.com`;
}

async function fillSignupForm(page, firstName, lastName, email) {
  log(`开始填写注册表单: ${firstName} ${lastName}, ${email}`);
  
  try {
    // 模拟真人输入延迟：先观察一下页面
    await page.waitForTimeout(randomDelay(500, 1000));
    
    // 填写 First name（像真人一样逐字符输入）
    const firstNameFilled = await humanLikeFill(
      page,
      'input[name="firstName"], input[placeholder*="first name" i], input#firstName',
      firstName,
      'First name'
    );
    
    // 填写完一项后，稍作停顿（模拟人移动到下一个字段）
    await page.waitForTimeout(randomDelay(300, 800));
    
    // 填写 Last name
    const lastNameFilled = await humanLikeFill(
      page,
      'input[name="lastName"], input[placeholder*="last name" i], input#lastName',
      lastName,
      'Last name'
    );
    
    // 稍作停顿
    await page.waitForTimeout(randomDelay(300, 800));
    
    // 填写 Email
    const emailFilled = await humanLikeFill(
      page,
      'input[name="email"], input[type="email"], input[placeholder*="email" i], input#email',
      email,
      'Email'
    );
    
    if (!firstNameFilled || !lastNameFilled || !emailFilled) {
      log('部分表单字段填写失败，可能页面结构不同');
      return false;
    }
    
    // 模拟人阅读协议的时间
    await page.waitForTimeout(randomDelay(1000, 2000));
    
    // 勾选同意协议复选框（更像真人的点击）
    try {
      const checkbox = page.locator('input[type="checkbox"], input[name*="agree"], input[name*="terms"]').first();
      if (await checkbox.isVisible({ timeout: 5000 }).catch(() => false)) {
        // 先悬停一下
        await checkbox.hover();
        await page.waitForTimeout(randomDelay(200, 500));
        await checkbox.click();
        log('✓ 已勾选同意协议');
      }
    } catch (e) {
      log('未找到或无需勾选协议复选框');
    }
    
    // 等待一下让按钮变为可点击状态，给用户时间看到填写完成
    await page.waitForTimeout(randomDelay(800, 1500));
    
    // 提示用户手动点击 Continue
    log('🖱️ 表单已填写完成，请手动点击 Continue 按钮');
    
    // 等待用户手动点击 Continue
    const result = await waitForUserClick(
      page,
      'button[type="submit"], button:has-text("Continue"), button:has-text("Sign up"), button.continue-btn, button.primary',
      'Continue',
      90000
    );
    
    if (result.success) {
      log('✓ 已提交注册表单');
      return true;
    }
    
    return false;
  } catch (error) {
    log(`填写表单出错: ${error.message}`);
    return false;
  }
}

// 生成随机密码（8-64位，包含字母和数字）
function generateRandomPassword() {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
  let password = '';
  // 生成12位密码
  for (let i = 0; i < 12; i++) {
    password += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  // 确保至少有一个数字和一个字母
  password += 'A' + '1';
  return password;
}

async function fillPasswordForm(page, password) {
  log(`开始填写密码表单，密码: ${password}`);
  
  try {
    // 等待密码输入框出现
    const passwordInput = page.locator('input[name="password"], input[type="password"], input[placeholder*="password" i], input[placeholder*="Create password" i]').first();
    if (!await passwordInput.isVisible({ timeout: 10000 }).catch(() => false)) {
      log('未找到密码输入框，可能页面跳转中');
      return false;
    }
    
    // 模拟真人思考时间
    await page.waitForTimeout(randomDelay(800, 1500));
    
    // 填写密码（像真人一样逐字符输入）
    const passwordFilled = await humanLikeFill(
      page,
      'input[name="password"], input[type="password"], input[placeholder*="password" i], input[placeholder*="Create password" i]',
      password,
      'Password'
    );
    
    // 移动到确认密码字段前的停顿
    await page.waitForTimeout(randomDelay(400, 900));
    
    // 填写确认密码
    const confirmFilled = await humanLikeFill(
      page,
      'input[name="passwordConfirmation"], input[name="confirmPassword"], input[placeholder*="confirm" i], input[placeholder*="Confirm password" i]',
      password,
      'Password confirmation'
    );
    
    if (!passwordFilled || !confirmFilled) {
      log('密码表单填写失败');
      return false;
    }
    
    // 模拟人阅读密码要求的时间
    await page.waitForTimeout(randomDelay(1000, 2000));
    
    // 提示用户手动点击 Continue
    log('🖱️ 密码已填写完成，请手动点击 Continue 按钮');
    
    // 等待用户手动点击 Continue
    const result = await waitForUserClick(
      page,
      'button[type="submit"], button:has-text("Continue"), button.continue-btn, button.primary, button:has-text("Create account")',
      'Continue',
      90000
    );
    
    if (result.success) {
      log('✓ 已提交密码表单');
      return true;
    }
    
    return false;
  } catch (error) {
    log(`填写密码表单出错: ${error.message}`);
    return false;
  }
}

// 等待人机验证完成并提示用户手动点击 Continue
async function waitForHumanVerification(page) {
  log('等待人机验证完成（最长90秒）...');
  
  const maxWaitTime = 90000; // 90秒
  const checkInterval = 5000; // 每5秒检测一次
  const startTime = Date.now();
  let verificationCompleted = false;
  
  while (Date.now() - startTime < maxWaitTime) {
    try {
      // 检测成功标志：绿色的 ✓ 或 "成功" 文本，或 Cloudflare 验证成功的标志
      const successIndicator = page.locator(
        '.success-checkmark, .check-icon, .success-icon, [class*="success"], ' +
        'text=成功, text=Success, text=✓, .cf-turnstile-success, ' +
        '[style*="green"], [style*="#4CAF50"]'
      ).first();
      
      const isSuccessVisible = await successIndicator.isVisible({ timeout: 1000 }).catch(() => false);
      
      if (isSuccessVisible) {
        if (!verificationCompleted) {
          log('✓ 人机验证已完成！请手动点击 Continue 按钮继续');
          verificationCompleted = true;
        }
        
        // 现在开始等待用户手动点击 Continue
        const result = await waitForUserClick(
          page,
          'button:has-text("Continue"), button[type="submit"], .continue-btn, button.primary',
          'Continue',
          maxWaitTime - (Date.now() - startTime) // 剩余时间
        );
        
        if (result.success) {
          log('✓ 已点击 Continue，继续后续流程');
          return true;
        }
        return false;
      }
    } catch (e) {
      // 检测失败，继续等待
    }
    
    const remainingSeconds = Math.floor((maxWaitTime - (Date.now() - startTime)) / 1000);
    const step = await detectCurrentStep(page);
    log(`验证未完成 | 当前步骤: ${step || '验证中'} | 剩余${remainingSeconds}秒 | 请完成人机验证`);
    
    await page.waitForTimeout(checkInterval);
  }
  
  log('人机验证等待超时（90秒）');
  return false;
}

// ==================== 主流程 ====================

async function main() {
  const proxyUrl = process.argv[2];
  const email = process.argv[3];
  const firstName = process.argv[4] || generateRandomName();
  const lastName = process.argv[5] || generateRandomName();
  const browserPath = process.argv[6]; // 浏览器路径（可选）
  
  log('========== Windsurf OAuth 授权 ==========');
  if (proxyUrl) {
    log(`代理: ${proxyUrl}`);
  }
  if (email) {
    log(`邮箱: ${email}`);
    log(`姓名: ${firstName} ${lastName}`);
  }
  if (browserPath) {
    log(`浏览器: ${browserPath}`);
  }
  
  // 启动回调服务器
  const serverInfo = await startCallbackServer();
  if (!serverInfo.port) {
    log('启动回调服务器失败');
    console.log(JSON.stringify({ success: false, error: '启动回调服务器失败' }));
    process.exit(1);
  }
  
  const callbackUrl = `http://127.0.0.1:${serverInfo.port}/windsurf-auth-callback`;
  
  // 构建授权 URL
  const authParams = new URLSearchParams({
    response_type: 'token',
    client_id: WINDSURF_CLIENT_ID,
    redirect_uri: callbackUrl,
    state: 'windsurf_auto_' + Date.now(),
    prompt: 'login',
    redirect_parameters_type: 'query',
    workflow: 'onboarding'
  });
  const authUrl = `${WINDSURF_AUTH_URL}?${authParams.toString()}`;
  
  log(`授权 URL: ${authUrl}`);
  
  // 确定要使用的浏览器
  let executablePath = null;
  let browserType = 'playwright';
  let isManualMode = false;
  
  if (browserPath) {
    if (browserPath === 'playwright' || browserPath === 'chromium') {
      browserType = 'playwright';
    } else if (browserPath === 'chrome') {
      browserType = 'chrome';
      executablePath = findSystemChrome();
      if (!executablePath) {
        log('未找到 Google Chrome，尝试使用 Playwright Chromium');
        browserType = 'playwright';
      }
    } else if (browserPath === 'edge') {
      browserType = 'edge';
      executablePath = findSystemEdge();
      if (!executablePath) {
        log('未找到 Microsoft Edge，尝试使用 Playwright Chromium');
        browserType = 'playwright';
      }
    } else if (browserPath === 'manual') {
      // 手动模式：用户自己打开浏览器，脚本只检测完成
      isManualMode = true;
      log('========================================');
      log('🔧 手动浏览器模式');
      log('========================================');
      log('请按以下步骤操作：');
      log('1. 打开您的浏览器（Chrome/Edge/Safari 等）');
      log('2. 访问以下 URL：');
      log(`   ${authUrl}`);
      log('3. 完成注册流程（填写表单、通过人机验证）');
      log('4. 授权完成后，按回车键继续...');
      log('========================================');
    } else {
      // 自定义路径
      browserType = 'custom';
      if (fs.existsSync(browserPath)) {
        executablePath = browserPath;
      } else {
        log(`指定的浏览器路径不存在: ${browserPath}，使用 Playwright Chromium`);
        browserType = 'playwright';
      }
    }
  }
  
  let browser;
  let page;
  
  // 手动模式：等待用户按回车键
  if (isManualMode) {
    const readline = require('readline');
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });
    
    await new Promise((resolve) => {
      rl.question('完成授权后请按回车键继续...', () => {
        rl.close();
        resolve();
      });
    });
    
    // 手动模式下，直接尝试从回调服务器获取结果
    // 这里简化处理，假设用户已完成授权
    log('等待检测授权结果...');
    
    // 等待回调结果（最长60秒）
    const manualResult = await new Promise((resolve) => {
      const timeout = setTimeout(() => {
        resolve({ success: false, error: '等待授权结果超时' });
      }, 60000);
      
      // 监听回调服务器的响应
      serverInfo.server.once('request', (req, res) => {
        clearTimeout(timeout);
        const parsedUrl = url.parse(req.url, true);
        if (parsedUrl.query.access_token) {
          resolve({
            success: true,
            accessToken: parsedUrl.query.access_token,
            tokenType: parsedUrl.query.token_type || 'Bearer',
            expiresIn: parsedUrl.query.expires_in
          });
        } else {
          resolve({ success: false, error: '未获取到 access_token' });
        }
      });
    });
    
    if (manualResult.success) {
      log('✓ 授权成功');
      console.log(JSON.stringify(manualResult));
      process.exit(0);
    } else {
      log(`✗ 授权失败: ${manualResult.error}`);
      console.log(JSON.stringify(manualResult));
      process.exit(1);
    }
  }
  
  // 如果使用 Playwright，检查/安装 Chromium
  if (browserType === 'playwright') {
    if (!checkChromiumInstalled()) {
      if (!installChromium()) {
        log('Playwright Chromium 安装失败，尝试查找系统 Chrome');
        executablePath = findSystemChrome();
        if (!executablePath) {
          log('未找到可用的 Chrome/Chromium');
          console.log(JSON.stringify({ success: false, error: '未找到可用的 Chrome/Chromium' }));
          process.exit(1);
        }
      }
    }
  }
  
  log(`使用浏览器: ${browserType}${executablePath ? ` (${executablePath})` : ''}`);
  
  try {
    // 启动浏览器
    const launchOptions = {
      headless: false,
      args: ['--disable-blink-features=AutomationControlled']
    };
    
    if (executablePath) {
      launchOptions.executablePath = executablePath;
    }
    
    if (proxyUrl) {
      launchOptions.proxy = { server: proxyUrl };
    }
    
    browser = await chromium.launch(launchOptions);
    
    const context = await browser.newContext({
      viewport: { width: 1280, height: 900 },
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    });
    
    page = await context.newPage();
    
    // 导航到授权页面
    log('正在打开 Windsurf 授权页面...');
    await page.goto(authUrl, { waitUntil: 'networkidle', timeout: 60000 });
    log('✓ 页面加载完成');

    // 检测是否在登录页，如果是则点击 Sign up 跳转到注册页
    try {
      const signUpLink = page.locator('a[href*="signup"], a:has-text("Sign up"), a:has-text("sign up"), a.windsurf-link:has-text("Sign up")').first();
      if (await signUpLink.isVisible({ timeout: 5000 }).catch(() => false)) {
        log('检测到登录页面，点击 Sign up 跳转到注册页面...');
        await signUpLink.click();
        await page.waitForTimeout(2000);
        log('✓ 已跳转到注册页面');
      }
    } catch (e) {
      // 没有找到 Sign up 链接，可能已经在注册页或页面结构不同
      log('未检测到 Sign up 链接，继续当前页面流程');
    }

    // 检测并填写注册表单
    const emailToUse = email || generateRandomEmail();
    const firstNameToUse = firstName || generateRandomName();
    const lastNameToUse = lastName || generateRandomName();
    const passwordToUse = generateRandomPassword();
    
    await fillSignupForm(page, firstNameToUse, lastNameToUse, emailToUse);
    
    // 填写密码表单
    await fillPasswordForm(page, passwordToUse);
    
    // 等待人机验证完成并点击 Continue
    await waitForHumanVerification(page);

    // 等待用户完成授权或检测重定向
    log('等待用户完成授权...');
    
    // 监听导航事件
    const tokenPromise = new Promise(async (resolve) => {
      // 等待回调服务器的响应
      const result = await new Promise((r) => {
        serverInfo.server.once('close', () => {
          // 服务器关闭意味着已经收到回调
        });
        // 设置超时
        setTimeout(() => r({ timeout: true }), 120000);
      });
      
      if (result && result.timeout) {
        resolve({ success: false, error: '授权超时' });
      }
    });
    
    // 同时监听页面 URL 变化
    const urlPromise = new Promise((resolve) => {
      const checkInterval = setInterval(async () => {
        const currentUrl = page.url();
        if (currentUrl.includes('windsurf-auth-callback') || currentUrl.startsWith('http://127.0.0.1')) {
          clearInterval(checkInterval);
          // 从 URL 解析 token
          const parsed = url.parse(currentUrl, true);
          if (parsed.query.access_token) {
            resolve({
              success: true,
              accessToken: parsed.query.access_token,
              tokenType: parsed.query.token_type || 'Bearer',
              expiresIn: parsed.query.expires_in
            });
          } else {
            resolve({ success: false, error: '授权失败' });
          }
        }
      }, 1000);
      
      // 120秒超时
      setTimeout(() => {
        clearInterval(checkInterval);
        resolve({ success: false, error: '授权超时' });
      }, 120000);
    });
    
    // 等待任一个完成
    const result = await Promise.race([tokenPromise, urlPromise]);
    
    await browser.close();
    browser = null;
    
    if (result.success) {
      log('✓ 成功获取 Access Token');
      console.log(JSON.stringify({
        success: true,
        accessToken: result.accessToken,
        tokenType: result.tokenType,
        expiresIn: result.expiresIn
      }));
    } else {
      log(`✗ 授权失败: ${result.error}`);
      console.log(JSON.stringify({ success: false, error: result.error }));
    }
    
  } catch (error) {
    log(`错误: ${error.message}`);
    if (browser) {
      await browser.close();
    }
    console.log(JSON.stringify({ success: false, error: error.message }));
    process.exit(1);
  }
}

main();
