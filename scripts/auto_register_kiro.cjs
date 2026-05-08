/**
 * Kiro 自动注册 Node.js 脚本
 * 使用 Playwright 实现 AWS Builder ID 自动注册
 * 完全参考 Kiro-auto-register 项目的 autoRegister.ts 实现
 * 
 * 参数: node auto_register_kiro.js <email> <password> <firstName> <lastName> [proxyUrl]
 */

const { chromium } = require('playwright');
const fs = require('fs');
const { execSync } = require('child_process');

const DEFAULT_PASSWORD = 'admin123456aA!';
const AWS_REGISTER_URL = 'https://view.awsapps.com/start/#/device?user_code=PQCF-FCCN';

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

// ==================== 页面操作辅助函数 ====================

/**
 * 等待元素可见并输入内容
 */
async function waitAndFill(page, selector, value, description, timeout = 30000) {
  log(`等待${description}出现...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout });
    await page.waitForTimeout(500);
    await element.clear();
    await element.fill(value);
    log(`✓ 已输入${description}: ${value}`);
    return true;
  } catch (error) {
    log(`✗ ${description}操作失败: ${error.message}`);
    return false;
  }
}

/**
 * 等待按钮可见并点击，带错误检测和重试
 */
async function waitAndClickWithRetry(page, selector, description, timeout = 30000, maxRetries = 3) {
  log(`等待${description}出现...`);
  try {
    const element = page.locator(selector).first();
    await element.waitFor({ state: 'visible', timeout });
    await page.waitForTimeout(500);
    await element.click();
    log(`✓ 已点击${description}`);
    
    // 检查是否有错误弹窗，如果有则重试
    for (let retry = 0; retry < maxRetries; retry++) {
      await page.waitForTimeout(1500);
      
      // 检查错误弹窗
      const errorSelectors = [
        'div.awsui_content_mx3cw_97dyn_391',
        '[class*="awsui_content_"]',
        '.awsui-flash-error',
        '[data-testid="flash-error"]'
      ];
      
      const errorTexts = [
        '抱歉，处理您的请求时出错',
        'Sorry, there was an error processing your request',
        'error processing your request',
        'Please try again',
        '请重试'
      ];
      
      let hasError = false;
      for (const errSelector of errorSelectors) {
        try {
          const errorElements = await page.locator(errSelector).all();
          for (const el of errorElements) {
            const text = await el.textContent();
            if (text && errorTexts.some(errText => text.includes(errText))) {
              hasError = true;
              log(`⚠ 检测到错误弹窗: "${text.substring(0, 50)}..."`);
              break;
            }
          }
        } catch {}
      }
      
      if (!hasError) {
        return true;
      }
      
      if (retry < maxRetries - 1) {
        log(`重试点击${description} (${retry + 2}/${maxRetries})...`);
        await page.waitForTimeout(2000);
        try {
          await element.click();
          log(`✓ 已重新点击${description}`);
        } catch (e) {
          log(`✗ 重新点击${description}失败: ${e.message}`);
        }
      }
    }
    
    log(`✗ ${description}多次重试后仍然失败`);
    return false;
  } catch (error) {
    log(`✗ ${description}点击失败: ${error.message}`);
    return false;
  }
}

// ==================== 验证码处理 ====================

/**
 * 获取验证码 - 通过临时文件与前端交互
 * 脚本发送 [NEED_CODE] 信号，前端显示输入框，用户输入后写入文件
 */
async function getVerificationCode(email, timeout = 120) {
  const fs = require('fs');
  const path = require('path');
  const os = require('os');
  
  // 创建临时文件路径
  const tempDir = os.tmpdir();
  const codeFilePath = path.join(tempDir, `aws_verification_code_${email.replace(/[^a-zA-Z0-9]/g, '_')}.txt`);
  const signalFilePath = path.join(tempDir, `aws_verification_signal_${email.replace(/[^a-zA-Z0-9]/g, '_')}.txt`);
  
  // 清理旧文件
  try {
    if (fs.existsSync(codeFilePath)) fs.unlinkSync(codeFilePath);
    if (fs.existsSync(signalFilePath)) fs.unlinkSync(signalFilePath);
  } catch (e) {}
  
  // 发送需要验证码的信号给前端
  log(`[NEED_CODE] ${email}`);
  log(`等待验证码文件: ${codeFilePath}`);
  log(`请在前端输入验证码，或创建文件: ${codeFilePath}`);
  
  // 创建信号文件（前端可以检测到这个文件）
  try {
    fs.writeFileSync(signalFilePath, JSON.stringify({
      email: email,
      timestamp: Date.now(),
      codeFile: codeFilePath
    }));
  } catch (e) {
    log(`创建信号文件失败: ${e.message}`);
  }
  
  // 轮询等待验证码文件
  const startTime = Date.now();
  const checkInterval = 1000; // 每秒检查一次
  
  while (Date.now() - startTime < timeout * 1000) {
    try {
      if (fs.existsSync(codeFilePath)) {
        const code = fs.readFileSync(codeFilePath, 'utf8').trim();
        if (code && /^\d{6}$/.test(code)) {
          log(`✓ 获取到验证码: ${code}`);
          // 清理文件
          try {
            fs.unlinkSync(codeFilePath);
            fs.unlinkSync(signalFilePath);
          } catch (e) {}
          return code;
        }
      }
    } catch (e) {}
    
    await new Promise(resolve => setTimeout(resolve, checkInterval));
  }
  
  // 超时，清理文件
  try {
    if (fs.existsSync(codeFilePath)) fs.unlinkSync(codeFilePath);
    if (fs.existsSync(signalFilePath)) fs.unlinkSync(signalFilePath);
  } catch (e) {}
  
  log('✗ 等待验证码超时');
  return null;
}

// ==================== 主注册流程 ====================

async function executeRegistrationFlow(page, email, fullName, emailPassword) {
  const password = DEFAULT_PASSWORD;
  
  // 步骤1: 进入页面并输入邮箱
  log('\n步骤1: 进入注册页面，输入邮箱...');
  
  // 等待邮箱输入框出现
  const emailInputSelector = 'input[placeholder="username@example.com"]';
  if (!await waitAndFill(page, emailInputSelector, email, '邮箱', 30000)) {
    throw new Error('未找到邮箱输入框');
  }
  
  await page.waitForTimeout(1000);
  
  // 点击第一个继续按钮
  const firstContinueSelector = 'button[data-testid="test-primary-button"]';
  if (!await waitAndClickWithRetry(page, firstContinueSelector, '第一个继续按钮')) {
    throw new Error('点击第一个继续按钮失败');
  }
  
  await page.waitForTimeout(3000);
  
  // 检测页面状态：新用户还是已注册用户
  const loginHeadingSelector = 'span[class*="awsui_heading-text"]:has-text("Sign in with your AWS Builder ID")';
  const verifyHeadingSelector = 'span[class*="awsui_heading-text"]:has-text("Verify")';
  const verifyCodeInputSelector = 'input[placeholder="6-digit"]';
  const nameInputSelector = 'input[placeholder="Maria José Silva"]';
  
  let isLoginFlow = false;
  let isVerifyFlow = false;
  
  try {
    // 等待其中一个元素出现
    const result = await Promise.race([
      page.locator(loginHeadingSelector).first().waitFor({ state: 'visible', timeout: 10000 }).then(() => 'login'),
      page.locator(verifyHeadingSelector).first().waitFor({ state: 'visible', timeout: 10000 }).then(() => 'verify'),
      page.locator(verifyCodeInputSelector).first().waitFor({ state: 'visible', timeout: 10000 }).then(() => 'verify-input'),
      page.locator(nameInputSelector).first().waitFor({ state: 'visible', timeout: 10000 }).then(() => 'register')
    ]);
    
    if (result === 'login') {
      isLoginFlow = true;
    } else if (result === 'verify' || result === 'verify-input') {
      isLoginFlow = true;
      isVerifyFlow = true;
    }
  } catch {
    // 如果都没找到，尝试单独检测
    try {
      await page.locator(loginHeadingSelector).first().waitFor({ state: 'visible', timeout: 3000 });
      isLoginFlow = true;
    } catch {
      try {
        const hasVerify = await page.locator(verifyHeadingSelector).first().isVisible().catch(() => false);
        const hasVerifyInput = await page.locator(verifyCodeInputSelector).first().isVisible().catch(() => false);
        if (hasVerify || hasVerifyInput) {
          isLoginFlow = true;
          isVerifyFlow = true;
        }
      } catch {
        isLoginFlow = false;
      }
    }
  }
  
  if (isLoginFlow) {
    // ========== 登录流程（邮箱已注册）==========
    if (isVerifyFlow) {
      log('\n⚠ 检测到验证页面，邮箱已注册，直接进入验证码步骤...');
    } else {
      log('\n⚠ 检测到邮箱已注册，切换到登录流程...');
    }
    
    // 如果不是直接验证流程，需要先输入密码
    if (!isVerifyFlow) {
      log('\n步骤2(登录): 输入密码...');
      const loginPasswordSelector = 'input[placeholder="Enter password"]';
      if (!await waitAndFill(page, loginPasswordSelector, password, '登录密码')) {
        throw new Error('未找到登录密码输入框');
      }
      
      await page.waitForTimeout(1000);
      
      const loginContinueSelector = 'button[data-testid="test-primary-button"]';
      if (!await waitAndClickWithRetry(page, loginContinueSelector, '登录继续按钮')) {
        throw new Error('点击登录继续按钮失败');
      }
      
      await page.waitForTimeout(3000);
    }
    
    // 步骤3(登录): 等待验证码输入框出现
    log('\n步骤3(登录): 获取并输入验证码...');
    const loginCodeSelectors = [
      'input[placeholder="6-digit"]',
      'input[placeholder="6 位数"]',
      'input[class*="awsui_input"][type="text"]'
    ];
    
    let loginCodeInput = null;
    for (const selector of loginCodeSelectors) {
      try {
        await page.locator(selector).first().waitFor({ state: 'visible', timeout: 10000 });
        loginCodeInput = selector;
        log('✓ 登录验证码输入框已出现');
        break;
      } catch {
        continue;
      }
    }
    
    if (!loginCodeInput) {
      throw new Error('未找到登录验证码输入框');
    }
    
    await page.waitForTimeout(1000);
    
    // 获取验证码
    const loginVerificationCode = await getVerificationCode(email, 120);
    if (!loginVerificationCode) {
      throw new Error('无法获取登录验证码');
    }
    
    // 输入验证码
    if (!await waitAndFill(page, loginCodeInput, loginVerificationCode, '登录验证码')) {
      throw new Error('输入登录验证码失败');
    }
    
    await page.waitForTimeout(1000);
    
    // 点击验证码确认按钮
    const loginVerifySelector = 'button[data-testid="test-primary-button"]';
    if (!await waitAndClickWithRetry(page, loginVerifySelector, '登录验证码确认按钮')) {
      throw new Error('点击登录验证码确认按钮失败');
    }
    
    await page.waitForTimeout(5000);
    
  } else {
    // ========== 注册流程（新账号）==========
    log('\n========== 新用户注册流程 ==========');
    
    // 步骤2: 输入姓名
    log('\n步骤2: 输入姓名...');
    if (!await waitAndFill(page, nameInputSelector, fullName, '姓名')) {
      throw new Error('未找到姓名输入框');
    }
    
    await page.waitForTimeout(1000);
    
    // 点击第二个继续按钮
    const secondContinueSelector = 'button[data-testid="signup-next-button"]';
    if (!await waitAndClickWithRetry(page, secondContinueSelector, '第二个继续按钮')) {
      throw new Error('点击第二个继续按钮失败');
    }
    
    await page.waitForTimeout(3000);
    
    // 步骤3: 等待验证码输入框出现
    log('\n步骤3: 获取并输入验证码...');
    const codeInputSelector = 'input[placeholder="6 位数"]';
    
    log('等待验证码输入框出现...');
    try {
      await page.locator(codeInputSelector).first().waitFor({ state: 'visible', timeout: 30000 });
      log('✓ 验证码输入框已出现');
    } catch {
      throw new Error('未找到验证码输入框');
    }
    
    await page.waitForTimeout(1000);
    
    // 获取验证码
    const verificationCode = await getVerificationCode(email, 120);
    if (!verificationCode) {
      throw new Error('无法获取验证码');
    }
    
    // 输入验证码
    if (!await waitAndFill(page, codeInputSelector, verificationCode, '验证码')) {
      throw new Error('输入验证码失败');
    }
    
    await page.waitForTimeout(1000);
    
    // 点击 Continue 按钮
    const verifyButtonSelector = 'button[data-testid="email-verification-verify-button"]';
    if (!await waitAndClickWithRetry(page, verifyButtonSelector, 'Continue 按钮')) {
      throw new Error('点击 Continue 按钮失败');
    }
    
    await page.waitForTimeout(3000);
    
    // 步骤4: 输入密码
    log('\n步骤4: 输入密码...');
    const passwordInputSelector = 'input[placeholder="Enter password"]';
    if (!await waitAndFill(page, passwordInputSelector, password, '密码')) {
      throw new Error('未找到密码输入框');
    }
    
    await page.waitForTimeout(500);
    
    // 输入确认密码
    const confirmPasswordSelector = 'input[placeholder="Re-enter password"]';
    if (!await waitAndFill(page, confirmPasswordSelector, password, '确认密码')) {
      throw new Error('未找到确认密码输入框');
    }
    
    await page.waitForTimeout(1000);
    
    // 点击第三个继续按钮
    const thirdContinueSelector = 'button[data-testid="test-primary-button"]';
    if (!await waitAndClickWithRetry(page, thirdContinueSelector, '第三个继续按钮')) {
      throw new Error('点击第三个继续按钮失败');
    }
    
    await page.waitForTimeout(5000);
  }
  
  return true;
}

// ==================== SSO Token 提取 ====================

async function extractSsoToken(page) {
  try {
    const cookies = await page.context().cookies();
    for (const cookie of cookies) {
      if (cookie.name.includes('x-amz-sso_authn')) {
        log(`找到 SSO Token: ${cookie.name.substring(0, 20)}...`);
        return cookie.value;
      }
    }
    
    // 也检查 localStorage
    try {
      const localStorage = await page.evaluate(() => {
        const data = {};
        for (let i = 0; i < localStorage.length; i++) {
          const key = localStorage.key(i);
          data[key] = localStorage.getItem(key);
        }
        return data;
      });
      
      for (const [key, value] of Object.entries(localStorage)) {
        if (key.includes('sso') || key.includes('auth')) {
          log(`localStorage 找到潜在 token: ${key}`);
        }
      }
    } catch (e) {}
    
    return null;
  } catch (e) {
    log(`提取 SSO Token 失败: ${e.message}`);
    return null;
  }
}

// ==================== 主函数 ====================

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 4) {
    console.error('用法: node auto_register_kiro.js <email> <password> <firstName> <lastName> [proxyUrl]');
    process.exit(1);
  }
  
  const [email, emailPassword, firstName, lastName, proxyUrl] = args;
  const fullName = `${firstName} ${lastName}`;
  
  log('========== 开始 AWS Builder ID 注册 ==========');
  log(`邮箱: ${email}`);
  log(`姓名: ${fullName}`);
  
  let browser;
  let result = { success: false, error: null, ssoToken: null, name: null };
  
  try {
    // 检测 Chromium 是否已安装
    log('\n准备: 检测 Chromium...');
    let chromiumInstalled = checkChromiumInstalled();
    
    if (!chromiumInstalled) {
      log('未找到 Playwright Chromium，尝试安装...');
      chromiumInstalled = installChromium();
    }
    
    // 启动浏览器配置
    const launchOptions = {
      headless: false,
      args: ['--disable-blink-features=AutomationControlled']
    };
    
    if (proxyUrl) {
      launchOptions.proxy = { server: proxyUrl };
      log(`使用代理: ${proxyUrl}`);
    }
    
    // 如果没有 Playwright Chromium，尝试使用系统 Chrome
    if (!chromiumInstalled) {
      const systemChrome = findSystemChrome();
      if (systemChrome) {
        log('使用系统 Chrome 作为替代...');
        launchOptions.executablePath = systemChrome;
      } else {
        throw new Error('未找到可用的浏览器。请安装 Chrome 或运行: npx playwright install chromium');
      }
    }
    
    log('\n启动浏览器...');
    browser = await chromium.launch(launchOptions);
    log('✓ 浏览器启动成功');
    
    // 创建上下文和页面
    const context = await browser.newContext({
      viewport: { width: 1280, height: 900 },
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    });
    const page = await context.newPage();
    
    // 导航到注册页面
    log('\n访问注册页面...');
    await page.goto(AWS_REGISTER_URL, { waitUntil: 'networkidle', timeout: 60000 });
    log('✓ 页面加载完成');
    await page.waitForTimeout(2000);
    
    // 执行注册流程
    const registrationSuccess = await executeRegistrationFlow(page, email, fullName, emailPassword);
    
    if (!registrationSuccess) {
      throw new Error('注册流程执行失败');
    }
    
    log('\n✓ 注册/登录流程执行完成');
    
    // 提取 SSO Token
    log('\n提取 SSO Token...');
    const ssoToken = await extractSsoToken(page);
    
    if (ssoToken) {
      result.success = true;
      result.ssoToken = ssoToken;
      result.name = fullName;
      log('✓ 成功获取 SSO Token');
    } else {
      log('⚠ 未能从 Cookie 中提取 SSO Token，但流程已完成');
      // 如果没有 token 但流程完成，也算部分成功
      result.success = true;
      result.name = fullName;
      result.error = '未能提取 SSO Token';
      
      // 延迟关闭浏览器，给用户 90 秒手动处理时间
      log('\n⏳ 浏览器将保持打开 90 秒，供您手动处理...');
      log('您可以在此期间手动完成注册流程，脚本会尝试再次提取 Token');
      
      // 等待 90 秒，期间每秒尝试重新提取一次
      const waitSeconds = 90;
      let tokenFound = false;
      for (let i = 0; i < waitSeconds; i++) {
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // 每秒尝试提取一次
        const retryToken = await extractSsoToken(page);
        if (retryToken) {
          result.ssoToken = retryToken;
          result.error = null;
          tokenFound = true;
          log(`✓ 延迟期间成功获取 SSO Token！(${i + 1}秒后)`);
          break;
        }
        
        // 每 10 秒输出一次提示
        if ((i + 1) % 10 === 0) {
          log(`⏳ 已等待 ${i + 1} 秒，还剩 ${waitSeconds - i - 1} 秒...`);
        }
      }
      
      if (!tokenFound) {
        log('\n⚠ 90 秒延迟结束，仍未获取到 SSO Token');
      }
    }
    
  } catch (error) {
    result.error = error.message;
    log(`\n✗ 错误: ${error.message}`);
  } finally {
    if (browser) {
      await browser.close();
      log('\n浏览器已关闭');
    }
  }
  
  log('========== 注册流程结束 ==========');
  
  // 输出 JSON 结果到 stdout
  console.log(JSON.stringify(result));
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
