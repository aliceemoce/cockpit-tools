/**
 * Windsurf 自动注册 - Stealth 模式
 * 使用 Playwright + Stealth 插件绕过 Cloudflare 检测
 * 
 * 参数: node auto_register_windsurf_stealth.js [proxyUrl] [email] [firstName] [lastName] [browserPath]
 */

const playwright = require('playwright');
const fs = require('fs');
const path = require('path');
const http = require('http');
const url = require('url');

const WINDSURF_AUTH_URL = 'https://www.windsurf.com/windsurf/signin';
const WINDSURF_CLIENT_ID = '3GUryQ7ldAeKEuD2obYnppsnmj58eP5u';

// 日志输出到 stderr（这样 stdout 可以保留给 JSON 结果）
function log(message) {
  console.error(`[LOG] ${message}`);
}

// 随机延迟
function randomDelay(min, max) {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

// 二次贝塞尔曲线
function quadraticBezier(p0, p1, p2, t) {
  return (1 - t) * (1 - t) * p0 + 2 * (1 - t) * t * p1 + t * t * p2;
}

// 模拟真人鼠标移动 - 使用贝塞尔曲线和自然抖动
async function humanLikeMouseMove(page, element) {
  const box = await element.boundingBox();
  if (!box) return;
  
  // 目标位置（元素中心附近随机偏移）
  const targetX = box.x + box.width / 2 + randomDelay(-15, 15);
  const targetY = box.y + box.height / 2 + randomDelay(-15, 15);
  
  // 获取当前鼠标位置
  const currentMouse = await page.evaluate(() => {
    return { x: window.mouseX || 0, y: window.mouseY || 0 };
  }).catch(() => ({ x: 0, y: 0 }));
  
  const startX = currentMouse.x || randomDelay(100, 400);
  const startY = currentMouse.y || randomDelay(100, 300);
  
  // 贝塞尔控制点（创建曲线）
  const controlX = (startX + targetX) / 2 + randomDelay(-100, 100);
  const controlY = (startY + targetY) / 2 + randomDelay(-100, 100);
  
  // 移动步骤数（根据距离调整）
  const distance = Math.sqrt(Math.pow(targetX - startX, 2) + Math.pow(targetY - startY, 2));
  const steps = Math.max(15, Math.min(40, Math.floor(distance / 20)));
  
  // 先快速移动到目标附近
  for (let i = 0; i <= steps * 0.7; i++) {
    const t = i / (steps * 0.7);
    const x = quadraticBezier(startX, controlX, targetX, t);
    const y = quadraticBezier(startY, controlY, targetY, t);
    
    // 添加随机抖动（模拟手部微颤）
    const jitterX = (Math.random() - 0.5) * 3;
    const jitterY = (Math.random() - 0.5) * 3;
    
    await page.mouse.move(x + jitterX, y + jitterY);
    
    // 非匀速移动：开始快，接近目标时慢
    const speed = 5 + (1 - t) * 15;
    await page.waitForTimeout(randomDelay(Math.floor(speed), Math.floor(speed + 10)));
  }
  
  // 在目标附近"犹豫"（真人会在点击前小幅移动）
  for (let i = 0; i < randomDelay(3, 6); i++) {
    const hesitateX = targetX + (Math.random() - 0.5) * 8;
    const hesitateY = targetY + (Math.random() - 0.5) * 8;
    await page.mouse.move(hesitateX, hesitateY);
    await page.waitForTimeout(randomDelay(30, 80));
  }
  
  // 最后精确移动到目标
  await page.mouse.move(targetX, targetY);
  await page.waitForTimeout(randomDelay(100, 250));
}

// 真实的人类点击（按下-停顿-释放）
async function humanLikeMouseClick(page, element) {
  await humanLikeMouseMove(page, element);
  
  // 获取元素位置
  const box = await element.boundingBox();
  if (!box) throw new Error('无法获取元素位置');
  
  const x = box.x + box.width / 2 + randomDelay(-5, 5);
  const y = box.y + box.height / 2 + randomDelay(-5, 5);
  
  // 移动到位置
  await page.mouse.move(x, y);
  await page.waitForTimeout(randomDelay(50, 150));
  
  // 按下鼠标
  await page.mouse.down();
  
  // 停顿（真人点击会有短暂停顿）
  await page.waitForTimeout(randomDelay(80, 200));
  
  // 释放鼠标
  await page.mouse.up();
  
  // 点击后轻微移动（真人点击后手会抖一下）
  await page.waitForTimeout(randomDelay(50, 120));
  await page.mouse.move(x + randomDelay(-3, 3), y + randomDelay(-3, 3));
}

// 像真人一样逐字符输入
async function humanLikeFill(page, selector, value, description) {
  log(`正在输入${description}...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout: 10000 });
    
    // 模拟真人鼠标移动并真实点击
    await humanLikeMouseClick(page, await element.elementHandle());
    await page.waitForTimeout(randomDelay(150, 400));
    
    // 清除现有内容
    await element.press('Control+a');
    await page.waitForTimeout(randomDelay(80, 200));
    await element.press('Delete');
    await page.waitForTimeout(randomDelay(100, 300));
    
    // 逐字符输入，带随机延迟和偶尔的错误修正
    for (let i = 0; i < value.length; i++) {
      const char = value[i];
      
      // 偶尔打错字并修正（1%概率）
      if (Math.random() > 0.99 && i > 0) {
        const wrongChar = String.fromCharCode(char.charCodeAt(0) + 1);
        await element.press(wrongChar);
        await page.waitForTimeout(randomDelay(100, 300));
        await element.press('Backspace');
        await page.waitForTimeout(randomDelay(80, 200));
      }
      
      await element.press(char);
      
      // 输入延迟：80-300ms，偶尔停顿更久
      let delay;
      if (Math.random() > 0.95) {
        // 5% 概率长停顿（思考时间）
        delay = randomDelay(500, 1200);
      } else if (Math.random() > 0.8) {
        // 20% 概率中等停顿
        delay = randomDelay(200, 400);
      } else {
        // 正常输入速度
        delay = randomDelay(80, 180);
      }
      await page.waitForTimeout(delay);
    }
    
    log(`✓ 已输入${description}`);
    return true;
  } catch (error) {
    log(`✗ ${description}输入失败: ${error.message}`);
    return false;
  }
}

// 像真人一样点击
async function humanLikeClick(page, selector, description) {
  log(`点击${description}...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout: 10000 });
    
    // 使用真实的鼠标移动和点击
    await humanLikeMouseClick(page, await element.elementHandle());
    log(`✓ 已点击${description}`);
    return true;
  } catch (error) {
    log(`✗ ${description}点击失败: ${error.message}`);
    return false;
  }
}

// 启动回调服务器
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

// 查找系统 Chrome
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

// 查找系统 Edge
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

// 获取 Chrome 用户数据目录
function getChromeUserDataDir() {
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    return path.join(localAppData, 'Google', 'Chrome', 'User Data');
  }
  return null;
}

// 创建临时用户数据目录
function createTempUserDataDir() {
  const tempDir = path.join(require('os').tmpdir(), 'chrome_stealth_' + Date.now());
  if (!fs.existsSync(tempDir)) {
    fs.mkdirSync(tempDir, { recursive: true });
  }
  return tempDir;
}

// 生成随机数据
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

function generateRandomPassword() {
  const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
  let password = '';
  for (let i = 0; i < 12; i++) {
    password += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  password += 'A' + '1';
  return password;
}

// 设置 stealth 脚本
async function applyStealthScripts(page) {
  // 隐藏 webdriver 标志
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'webdriver', {
      get: () => undefined
    });
    
    // 覆盖 permissions API
    const originalQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = (parameters) => (
      parameters.name === 'notifications' ?
        Promise.resolve({ state: Notification.permission }) :
        originalQuery(parameters)
    );
    
    // 模拟真实的插件列表
    Object.defineProperty(navigator, 'plugins', {
      get: () => [
        {
          0: { type: "application/x-google-chrome-pdf", suffixes: "pdf", description: "Portable Document Format" },
          description: "Portable Document Format",
          filename: "internal-pdf-viewer",
          length: 1,
          name: "Chrome PDF Plugin"
        },
        {
          0: { type: "application/pdf", suffixes: "pdf", description: "" },
          description: "Portable Document Format plugin",
          filename: "internal-pdf-viewer2",
          length: 1,
          name: "Chrome PDF Viewer"
        }
      ]
    });
    
    // 模拟真实的 mimeTypes
    Object.defineProperty(navigator, 'mimeTypes', {
      get: () => [
        { type: "application/pdf", suffixes: "pdf", description: "Portable Document Format", enabledPlugin: {} },
        { type: "application/x-google-chrome-pdf", suffixes: "pdf", description: "Portable Document Format", enabledPlugin: {} }
      ]
    });
    
    // 覆盖 chrome 对象
    window.chrome = { runtime: {} };
    
    // 覆盖 Notification API
    const originalNotification = window.Notification;
    Object.defineProperty(window, 'Notification', {
      get: () => originalNotification,
      set: (value) => {}
    });
    
    // 模拟真实的屏幕尺寸
    Object.defineProperty(window.screen, 'width', { get: () => 1920 });
    Object.defineProperty(window.screen, 'height', { get: () => 1080 });
    Object.defineProperty(window.screen, 'availWidth', { get: () => 1920 });
    Object.defineProperty(window.screen, 'availHeight', { get: () => 1040 });
    Object.defineProperty(window.screen, 'colorDepth', { get: () => 24 });
    Object.defineProperty(window.screen, 'pixelDepth', { get: () => 24 });
    
    // 模拟真实的时区
    const originalDate = Date;
    Date = class extends originalDate {
      constructor(...args) {
        super(...args);
      }
      getTimezoneOffset() {
        return -480; // 东八区
      }
    };
    
    // 覆盖 toString 方法防止检测
    const originalToString = Function.prototype.toString;
    Function.prototype.toString = function() {
      if (this === window.navigator.permissions.query) {
        return 'function query() { [native code] }';
      }
      return originalToString.call(this);
    };
  });
  
  // 设置 User-Agent
  await page.setExtraHTTPHeaders({
    'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8'
  });
}

// 等待用户点击（检测按钮消失或页面跳转）
async function waitForUserClick(page, buttonSelector, description, maxWaitMs = 90000) {
  log(`⏳ 请手动点击【${description}】（最长等待${maxWaitMs/1000}秒）...`);
  
  const fastCheckInterval = 500;
  const logInterval = 5000;
  let lastUrl = page.url();
  let lastLogTime = 0;
  
  const startTime = Date.now();
  
  while (Date.now() - startTime < maxWaitMs) {
    const currentUrl = page.url();
    const remainingSeconds = Math.floor((maxWaitMs - (Date.now() - startTime)) / 1000);
    const elapsed = Date.now() - startTime;
    
    if (currentUrl !== lastUrl) {
      log(`✓ 检测到页面跳转`);
      return { success: true, navigated: true };
    }
    
    const buttonVisible = await page.locator(buttonSelector).isVisible().catch(() => false);
    if (!buttonVisible) {
      await page.waitForTimeout(300);
      const stillNotVisible = await page.locator(buttonSelector).isVisible().catch(() => false);
      if (!stillNotVisible) {
        log(`✓ 检测到【${description}】已被点击`);
        return { success: true, navigated: false };
      }
    }
    
    if (elapsed - lastLogTime >= logInterval) {
      log(`⏳ 等待中... 剩余${remainingSeconds}秒 | 请手动点击【${description}】`);
      lastLogTime = elapsed;
    }
    
    await page.waitForTimeout(fastCheckInterval);
  }
  
  log(`✗ 等待【${description}】点击超时`);
  return { success: false, error: '等待用户点击超时' };
}

// 填写注册表单
async function fillSignupForm(page, firstName, lastName, email) {
  log(`开始填写注册表单: ${firstName} ${lastName}, ${email}`);
  
  try {
    await page.waitForTimeout(randomDelay(800, 1500));
    
    const firstNameFilled = await humanLikeFill(
      page,
      'input[name="firstName"], input[placeholder*="first name" i]',
      firstName,
      'First name'
    );
    
    await page.waitForTimeout(randomDelay(400, 900));
    
    const lastNameFilled = await humanLikeFill(
      page,
      'input[name="lastName"], input[placeholder*="last name" i]',
      lastName,
      'Last name'
    );
    
    await page.waitForTimeout(randomDelay(400, 900));
    
    const emailFilled = await humanLikeFill(
      page,
      'input[name="email"], input[type="email"]',
      email,
      'Email'
    );
    
    if (!firstNameFilled || !lastNameFilled || !emailFilled) {
      log('部分表单字段填写失败');
      return false;
    }
    
    await page.waitForTimeout(randomDelay(1000, 2000));
    
    try {
      const checkbox = page.locator('input[type="checkbox"]').first();
      if (await checkbox.isVisible({ timeout: 3000 }).catch(() => false)) {
        await humanLikeClick(page, 'input[type="checkbox"]', '同意协议复选框');
        log('✓ 已勾选同意协议');
      }
    } catch (e) {}
    
    await page.waitForTimeout(randomDelay(800, 1500));
    
    log('🖱️ 表单填写完成，请手动点击 Continue 按钮');
    
    const result = await waitForUserClick(
      page,
      'button[type="submit"], button:has-text("Continue")',
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

// 填写密码表单
async function fillPasswordForm(page, password) {
  log(`开始填写密码表单`);
  
  try {
    const passwordInput = page.locator('input[type="password"]').first();
    if (!await passwordInput.isVisible({ timeout: 10000 }).catch(() => false)) {
      log('未找到密码输入框');
      return false;
    }
    
    await page.waitForTimeout(randomDelay(800, 1500));
    
    const passwordFilled = await humanLikeFill(
      page,
      'input[type="password"]',
      password,
      'Password'
    );
    
    await page.waitForTimeout(randomDelay(400, 900));
    
    const confirmInputs = await page.locator('input[type="password"]').count();
    if (confirmInputs >= 2) {
      await humanLikeFill(
        page,
        'input[type="password"] >> nth=1',
        password,
        'Password confirmation'
      );
    }
    
    await page.waitForTimeout(randomDelay(1000, 2000));
    
    log('🖱️ 密码填写完成，请手动点击 Continue 按钮');
    
    const result = await waitForUserClick(
      page,
      'button[type="submit"], button:has-text("Continue")',
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

// 主函数
async function main() {
  const proxyUrl = process.argv[2];
  const email = process.argv[3];
  const firstName = process.argv[4] || generateRandomName();
  const lastName = process.argv[5] || generateRandomName();
  const browserPath = process.argv[6];
  
  log('========== Windsurf OAuth 授权 (Stealth 模式) ==========');
  
  const serverInfo = await startCallbackServer();
  if (!serverInfo.port) {
    log('启动回调服务器失败');
    console.log(JSON.stringify({ success: false, error: '启动回调服务器失败' }));
    process.exit(1);
  }
  
  const callbackUrl = `http://127.0.0.1:${serverInfo.port}/windsurf-auth-callback`;
  
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
  
  // 确定浏览器
  let executablePath = findSystemChrome();
  if (!executablePath) {
    executablePath = findSystemEdge();
  }
  if (!executablePath && browserPath && fs.existsSync(browserPath)) {
    executablePath = browserPath;
  }
  
  if (!executablePath) {
    log('未找到可用的浏览器');
    console.log(JSON.stringify({ success: false, error: '未找到可用的浏览器' }));
    process.exit(1);
  }
  
  log(`使用浏览器: ${executablePath}`);
  
  let browser;
  let page;
  
  // 尝试使用临时用户数据目录（避免 Chrome 已在运行的问题）
  const tempUserDataDir = createTempUserDataDir();
  log(`使用临时用户数据目录: ${tempUserDataDir}`);
  
  try {
    // 首先尝试使用临时的持久化上下文
    browser = await playwright.chromium.launchPersistentContext(tempUserDataDir, {
      headless: false,
      executablePath: executablePath,
      viewport: { width: 1920, height: 1080 },
      args: [
        '--disable-blink-features=AutomationControlled',
        '--disable-web-security',
        '--disable-features=IsolateOrigins,site-per-process',
        '--disable-site-isolation-trials',
        '--disable-extensions',
        '--disable-default-apps',
        '--disable-component-extensions-with-background-pages',
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        '--disable-features=TranslateUI',
        '--disable-ipc-flooding-protection',
        '--enable-features=NetworkService,NetworkServiceInProcess',
        '--force-color-profile=srgb',
        '--window-size=1920,1080',
        '--start-maximized',
        '--no-first-run',
        '--no-default-browser-check'
      ],
      ignoreDefaultArgs: ['--enable-automation', '--disable-infobars'],
      proxy: proxyUrl ? { server: proxyUrl } : undefined
    });
    
    page = await browser.newPage();
    log('浏览器启动成功（持久化上下文）');
  } catch (e) {
    log(`持久化上下文启动失败: ${e.message}，尝试普通启动...`);
    
    // 如果失败，尝试普通启动（无持久化上下文）
    try {
      browser = await playwright.chromium.launch({
        headless: false,
        executablePath: executablePath,
        viewport: { width: 1920, height: 1080 },
        args: [
          '--disable-blink-features=AutomationControlled',
          '--disable-web-security',
          '--disable-features=IsolateOrigins,site-per-process',
          '--no-first-run',
          '--no-default-browser-check',
          '--window-size=1920,1080',
          '--start-maximized'
        ],
        ignoreDefaultArgs: ['--enable-automation', '--disable-infobars'],
        proxy: proxyUrl ? { server: proxyUrl } : undefined
      });
      
      page = await browser.newPage();
      log('浏览器启动成功（普通模式）');
    } catch (e2) {
      log(`浏览器启动失败: ${e2.message}`);
      console.log(JSON.stringify({ success: false, error: `浏览器启动失败: ${e2.message}` }));
      process.exit(1);
    }
  }
  
  // 应用 stealth 脚本
  await applyStealthScripts(page);
  
  // 随机浏览行为 - 模拟真人四处看看
  async function randomBrowsing(page) {
    log('模拟浏览行为...');
    // 随机移动鼠标到几个位置
    for (let i = 0; i < randomDelay(2, 4); i++) {
      const x = randomDelay(200, 800);
      const y = randomDelay(200, 600);
      await page.mouse.move(x, y);
      await page.waitForTimeout(randomDelay(500, 1500));
    }
  }
  
  try {
    log('正在打开 Windsurf 授权页面...');
    await page.goto(authUrl, { waitUntil: 'networkidle', timeout: 60000 });
    log('✓ 页面加载完成');
    
    // 页面加载后先浏览一下
    await randomBrowsing(page);
    await page.waitForTimeout(randomDelay(1000, 2500));
    
    // 检测是否在登录页
    try {
      const signUpLink = page.locator('a:has-text("Sign up")').first();
      if (await signUpLink.isVisible({ timeout: 5000 }).catch(() => false)) {
        log('点击 Sign up 跳转到注册页面...');
        await humanLikeClick(page, 'a:has-text("Sign up")', 'Sign up');
        await page.waitForTimeout(randomDelay(2000, 3000));
      }
    } catch (e) {}
    
    const emailToUse = email || generateRandomEmail();
    const passwordToUse = generateRandomPassword();
    
    await fillSignupForm(page, firstName, lastName, emailToUse);
    
    await page.waitForTimeout(randomDelay(2000, 3000));
    
    await fillPasswordForm(page, passwordToUse);
    
    log('⏳ 等待人机验证完成...');
    log('💡 提示：如果出现人机验证，请手动完成');
    
    await page.waitForTimeout(5000);
    
    log('🖱️ 完成验证后，请手动点击 Continue 按钮');
    
    const result = await waitForUserClick(
      page,
      'button:has-text("Continue"), button[type="submit"]',
      'Continue',
      120000
    );
    
    if (result.success) {
      log('等待授权结果...');
      
      const authResult = await new Promise((resolve) => {
        const timeout = setTimeout(() => {
          resolve({ success: false, error: '授权超时' });
        }, 60000);
        
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
      
      await browser.close();
      
      if (authResult.success) {
        log('✓ 授权成功');
        console.log(JSON.stringify(authResult));
        process.exit(0);
      } else {
        log(`✗ 授权失败: ${authResult.error}`);
        console.log(JSON.stringify(authResult));
        process.exit(1);
      }
    }
    
    await browser.close();
    log('✗ 等待点击超时');
    console.log(JSON.stringify({ success: false, error: '等待点击超时' }));
    process.exit(1);
    
  } catch (error) {
    log(`错误: ${error.message}`);
    await browser.close();
    console.log(JSON.stringify({ success: false, error: error.message }));
    process.exit(1);
  }
}

main();
