import { useRef, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  Play,
  Square,
  Trash2,
  CheckCircle,
  XCircle,
  Clock,
  Loader2,
  Mail,
  Key,
  AlertCircle,
  Terminal
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAutoRegisterStore, type RegisterAccount } from '../stores/useAutoRegisterStore';
import {
  autoRegisterKiro,
  autoRegisterWindsurf,
  importFromSsoToken,
  stopAutoRegister,
  type AutoRegisterResult,
} from '../services/autoRegisterService';
import './AutoRegisterPage.css';

// 默认密码
const DEFAULT_PASSWORD = 'admin123456aA!';

// First Name 词汇组
const FIRST_NAMES = [
  'James', 'John', 'Robert', 'Michael', 'William', 'David', 'Richard', 'Joseph',
  'Thomas', 'Charles', 'Christopher', 'Daniel', 'Matthew', 'Anthony', 'Mark',
  'Donald', 'Steven', 'Paul', 'Andrew', 'Kenneth', 'Joshua', 'Kevin', 'Brian',
  'George', 'Timothy', 'Ronald', 'Edward', 'Jason', 'Jeffrey', 'Ryan',
  'Jacob', 'Gary', 'Nicholas', 'Eric', 'Jonathan', 'Stephen', 'Larry', 'Justin',
  'Scott', 'Brandon', 'Benjamin', 'Samuel', 'Gregory', 'Frank', 'Alexander',
  'Raymond', 'Patrick', 'Jack', 'Dennis', 'Jerry', 'Tyler', 'Aaron', 'Jose',
  'Adam', 'Nathan', 'Henry', 'Douglas', 'Zachary', 'Peter', 'Kyle', 'Ethan',
  'Walter', 'Noah', 'Jeremy', 'Christian', 'Keith', 'Roger', 'Terry', 'Gerald',
  'Harold', 'Sean', 'Austin', 'Carl', 'Arthur', 'Lawrence', 'Dylan', 'Jesse',
  'Jordan', 'Bryan', 'Billy', 'Joe', 'Bruce', 'Gabriel', 'Logan', 'Albert',
  'Willie', 'Alan', 'Juan', 'Wayne', 'Elijah', 'Randy', 'Roy', 'Vincent',
  'Ralph', 'Eugene', 'Russell', 'Bobby', 'Mason', 'Philip', 'Louis', 'Mary',
  'Patricia', 'Jennifer', 'Linda', 'Elizabeth', 'Barbara', 'Susan', 'Jessica',
  'Sarah', 'Karen', 'Nancy', 'Lisa', 'Betty', 'Margaret', 'Sandra', 'Ashley',
  'Kimberly', 'Emily', 'Donna', 'Michelle', 'Dorothy', 'Carol', 'Amanda',
  'Melissa', 'Deborah', 'Stephanie', 'Rebecca', 'Laura', 'Sharon', 'Cynthia',
  'Kathleen', 'Amy', 'Shirley', 'Angela', 'Helen', 'Anna', 'Brenda', 'Pamela',
  'Nicole', 'Emma', 'Samantha', 'Katherine', 'Christine', 'Debra', 'Rachel',
  'Catherine', 'Carolyn', 'Janet', 'Ruth', 'Maria', 'Heather', 'Diane', 'Virginia',
  'Julie', 'Joyce', 'Victoria', 'Olivia', 'Kelly', 'Christina', 'Lauren',
  'Joan', 'Evelyn', 'Judith', 'Megan', 'Cheryl', 'Andrea', 'Hannah', 'Martha',
  'Jacqueline', 'Frances', 'Gloria', 'Ann', 'Teresa', 'Kathryn', 'Sara',
  'Janice', 'Jean', 'Alice', 'Madison', 'Doris', 'Abigail', 'Julia', 'Judy',
  'Grace', 'Denise', 'Amber', 'Marilyn', 'Beverly', 'Danielle', 'Theresa',
  'Sophia', 'Marie', 'Diana', 'Brittany', 'Natalie', 'Isabella', 'Charlotte',
  'Rose', 'Alexis', 'Kayla', 'Liam', 'Emma', 'Noah', 'Olivia', 'Ava'
];

// Last Name 词汇组
const LAST_NAMES = [
  'Smith', 'Johnson', 'Williams', 'Brown', 'Jones', 'Garcia', 'Miller', 'Davis',
  'Rodriguez', 'Martinez', 'Hernandez', 'Lopez', 'Gonzalez', 'Wilson', 'Anderson',
  'Thomas', 'Taylor', 'Moore', 'Jackson', 'Martin', 'Lee', 'Perez', 'Thompson',
  'White', 'Harris', 'Sanchez', 'Clark', 'Ramirez', 'Lewis', 'Robinson', 'Walker',
  'Young', 'Allen', 'King', 'Wright', 'Scott', 'Torres', 'Nguyen', 'Hill',
  'Flores', 'Green', 'Adams', 'Nelson', 'Baker', 'Hall', 'Rivera', 'Campbell',
  'Mitchell', 'Carter', 'Roberts', 'Gomez', 'Phillips', 'Evans', 'Turner',
  'Diaz', 'Parker', 'Cruz', 'Edwards', 'Collins', 'Reyes', 'Stewart', 'Morris',
  'Morales', 'Murphy', 'Cook', 'Rogers', 'Gutierrez', 'Ortiz', 'Morgan',
  'Cooper', 'Peterson', 'Bailey', 'Reed', 'Kelly', 'Howard', 'Ramos', 'Kim',
  'Cox', 'Ward', 'Richardson', 'Watson', 'Brooks', 'Chavez', 'Wood', 'James',
  'Bennett', 'Gray', 'Mendoza', 'Ruiz', 'Hughes', 'Price', 'Alvarez', 'Castillo',
  'Sanders', 'Patel', 'Myers', 'Long', 'Ross', 'Foster', 'Jimenez', 'Powell',
  'Jenkins', 'Perry', 'Russell', 'Sullivan', 'Bell', 'Coleman', 'Butler',
  'Henderson', 'Barnes', 'Gonzales', 'Fisher', 'Vasquez', 'Simmons', 'Romero',
  'Jordan', 'Patterson', 'Alexander', 'Hamilton', 'Graham', 'Reynolds', 'Griffin',
  'Wallace', 'Moreno', 'West', 'Cole', 'Hayes', 'Bryant', 'Herrera', 'Gibson',
  'Ellis', 'Tran', 'Medina', 'Aguilar', 'Stevens', 'Murray', 'Ford', 'Castro',
  'Marshall', 'Owens', 'Harrison', 'Fernandez', 'Woods', 'Washington', 'Kennedy',
  'Wells', 'Vargas', 'Henry', 'Chen', 'Freeman', 'Webb', 'Tucker', 'Guzman',
  'Burns', 'Crawford', 'Olson', 'Simpson', 'Porter', 'Hunter', 'Gordon',
  'Mendez', 'Silva', 'Shaw', 'Snyder', 'Mason', 'Dixon', 'Munoz', 'Hunt',
  'Hicks', 'Holmes', 'Palmer', 'Wagner', 'Black', 'Robertson', 'Boyd', 'Rose',
  'Stone', 'Salazar', 'Fox', 'Warren', 'Mills', 'Meyer', 'Rice', 'Schmidt',
  'Garza', 'Daniels', 'Ferguson', 'Nichols', 'Stephens', 'Soto', 'Weaver',
  'Ryan', 'Gardner', 'Payne', 'Grant', 'Dunn', 'Kelley', 'Spencer', 'Hawkins',
  'Arnold', 'Pierce', 'Vazquez', 'Hansen', 'Peters', 'Santos', 'Hart',
  'Bradley', 'Knight', 'Elliott', 'Cunningham', 'Duncan', 'Armstrong', 'Hudson',
  'Carroll', 'Lane', 'Riley', 'Andrews', 'Alvarado', 'Ray', 'Delgado', 'Berry',
  'Perkins', 'Hoffman', 'Johnston', 'Matsumoto', 'Tanaka', 'Suzuki', 'Watanabe',
  'Takahashi', 'Ito', 'Yamamoto', 'Nakamura', 'Kobayashi', 'Yoshida', 'Yamada'
];

// 平台类型列表（对应 QuickSettingsType）
const PLATFORM_OPTIONS = [
  { value: 'antigravity', label: 'Antigravity' },
  { value: 'codex', label: 'Codex' },
  { value: 'github_copilot', label: 'GitHub Copilot' },
  { value: 'windsurf', label: 'Windsurf' },
  { value: 'kiro', label: 'Kiro' },
  { value: 'cursor', label: 'Cursor' },
  { value: 'gemini', label: 'Gemini' },
  { value: 'codebuddy', label: 'CodeBuddy' },
  { value: 'codebuddy_cn', label: 'CodeBuddy CN' },
  { value: 'qoder', label: 'Qoder' },
  { value: 'trae', label: 'Trae' },
  { value: 'workbuddy', label: 'WorkBuddy' },
  { value: 'zed', label: 'Zed' },
];

// 浏览器选项
const BROWSER_OPTIONS = [
  { value: 'playwright', label: 'Playwright (默认)' },
  { value: 'stealth', label: 'Stealth 模式 (防检测)' },
  { value: 'chrome', label: 'Google Chrome' },
  { value: 'edge', label: 'Microsoft Edge' },
  { value: 'manual', label: '手动浏览器 (绕过验证)' },
  { value: 'semi-auto', label: '半自动 (显示信息复制)' },
  { value: 'custom', label: '自定义浏览器路径...' },
];

export function AutoRegisterPage() {
  const { t } = useTranslation();
  const logEndRef = useRef<HTMLDivElement>(null);
  const [selectedPlatform, setSelectedPlatform] = useState('kiro');
  const [selectedBrowser, setSelectedBrowser] = useState('playwright');
  const [customBrowserPath, setCustomBrowserPath] = useState('');
  const [showCustomBrowserInput, setShowCustomBrowserInput] = useState(false);
  
  // 默认邮箱域名配置（从 localStorage 读取）
  const [defaultEmailDomain, setDefaultEmailDomain] = useState(() => {
    return localStorage.getItem('defaultEmailDomain') || 'annn.online';
  });
  
  // 保存默认邮箱域名到 localStorage
  const saveDefaultEmailDomain = (domain: string) => {
    const cleanDomain = domain.trim().replace(/^@/, ''); // 移除开头的 @
    setDefaultEmailDomain(cleanDomain);
    localStorage.setItem('defaultEmailDomain', cleanDomain);
    addLog(`默认邮箱域名已设置为: @${cleanDomain}`);
  };
  
  // 验证码输入相关状态
  const [verificationCode, setVerificationCode] = useState('');
  const [showCodeInput, setShowCodeInput] = useState(false);
  const [pendingEmail, setPendingEmail] = useState<string | null>(null);
  const codeInputRef = useRef<HTMLInputElement>(null);

  // Windsurf 半自动模式信息展示
  const [showWindsurfInfo, setShowWindsurfInfo] = useState(false);
  const [windsurfInfo, setWindsurfInfo] = useState<{
    email: string;
    firstName: string;
    lastName: string;
    password: string;
    registerUrl: string;
  } | null>(null);

  // 复制到剪贴板
  const copyToClipboard = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      addLog(`✓ ${label} 已复制到剪贴板`);
    } catch (err) {
      addLog(`✗ 复制失败: ${err}`);
    }
  };

  // 打开 Windsurf 注册页面（使用系统默认浏览器）
  const openWindsurfRegister = async () => {
    const url = 'https://www.windsurf.com/windsurf/signup';
    try {
      await openUrl(url);
      addLog('已使用系统默认浏览器打开 Windsurf 注册页面');
    } catch (err) {
      addLog(`打开浏览器失败: ${err}`);
      // 降级方案：使用 window.open
      window.open(url, '_blank');
    }
  };

  const {
    accounts,
    isRunning,
    logs,
    addAccounts,
    clearAccounts,
    updateAccountStatus,
    addLog,
    clearLogs,
    setIsRunning,
    requestStop,
    resetStop,
    getStats
  } = useAutoRegisterStore();

  // 自动滚动到日志底部（仅在验证码输入框隐藏时）
  useEffect(() => {
    if (!showCodeInput) {
      logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, showCodeInput]);
  
  // 监听自动注册日志事件，检测 [NEED_CODE] 信号
  useEffect(() => {
    const unlisten = listen('auto-register-log', (event: any) => {
      const { email: logEmail, message } = event.payload;
      
      // 将日志添加到 store 显示
      if (message) {
        addLog(`[${logEmail}] ${message}`);
      }
      
      // 检测是否需要验证码 - 支持两种格式：
      // 1. [NEED_CODE] email
      // 2. 消息包含 [NEED_CODE] email
      if (message && message.includes('[NEED_CODE]')) {
        const needEmail = message.includes(']') 
          ? message.split(']').pop()?.trim() || logEmail
          : logEmail;
        setPendingEmail(needEmail);
        setShowCodeInput(true);
        setVerificationCode('');
        // 自动聚焦到输入框
        setTimeout(() => {
          codeInputRef.current?.focus();
        }, 100);
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, [addLog]);
  
  // 验证码输入框自动聚焦
  useEffect(() => {
    if (showCodeInput && codeInputRef.current) {
      codeInputRef.current.focus();
    }
  }, [showCodeInput]);

  // 随机选择一个First Name
  const getRandomFirstName = (): string => {
    return FIRST_NAMES[Math.floor(Math.random() * FIRST_NAMES.length)];
  };

  // 随机选择一个Last Name
  const getRandomLastName = (): string => {
    return LAST_NAMES[Math.floor(Math.random() * LAST_NAMES.length)];
  };

  // 只解析邮箱，自动生成其他信息
  const parseAccounts = (text: string): RegisterAccount[] => {
    const lines = text.trim().split(/[\n,]+/); // 支持换行或逗号分隔
    const parsed: RegisterAccount[] = [];

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;

      // 提取邮箱（支持空格分隔的多个邮箱）
      const emailMatches = trimmed.match(/[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g);
      if (emailMatches) {
        for (const email of emailMatches) {
          parsed.push({
            id: crypto.randomUUID(),
            email: email.toLowerCase().trim(),
            password: DEFAULT_PASSWORD,
            firstName: getRandomFirstName(),
            lastName: getRandomLastName(),
            status: 'pending'
          });
        }
      }
    }

    return parsed;
  };

  // 生成随机邮箱（使用配置的默认域名）
  const generateRandomEmail = (): string => {
    const letters = 'abcdefghijklmnopqrstuvwxyz';
    const alphanumeric = 'abcdefghijklmnopqrstuvwxyz0123456789';
    let prefix = letters.charAt(Math.floor(Math.random() * letters.length));
    for (let i = 0; i < 10; i++) {
      prefix += alphanumeric.charAt(Math.floor(Math.random() * alphanumeric.length));
    }
    return `${prefix}@${defaultEmailDomain}`;
  };

  const handleClear = () => {
    if (isRunning) {
      alert(t('autoRegister.stopFirst'));
      return;
    }
    clearAccounts();
  };
  
  // 提交验证码
  const submitVerificationCode = async () => {
    if (!verificationCode || verificationCode.length !== 6 || !pendingEmail) {
      return;
    }
    
    try {
      // 调用 Rust 命令写入验证码到文件（注意参数包装格式）
      await invoke('submit_verification_code', {
        params: {
          email: pendingEmail,
          code: verificationCode
        }
      });
      
      addLog(`[${pendingEmail}] 验证码已提交: ${verificationCode}`);
      setShowCodeInput(false);
      setVerificationCode('');
      setPendingEmail(null);
    } catch (error) {
      addLog(`[${pendingEmail}] ✗ 提交验证码失败: ${error}`);
    }
  };
  
  // 取消验证码输入
  const cancelVerificationCode = () => {
    setShowCodeInput(false);
    setVerificationCode('');
    setPendingEmail(null);
  };

  // 固定代理地址
  const PROXY_URL = 'http://127.0.0.1:7890';

  // 单个账号注册任务
  const registerSingleAccount = async (account: RegisterAccount): Promise<void> => {
    if (useAutoRegisterStore.getState().shouldStop) return;
    if (account.status === 'success' || account.status === 'exists') return;

    try {
      updateAccountStatus(account.id, { status: 'registering' });
      addLog(`[${account.email}] ${t('autoRegister.starting')}...`);

      // 根据选中的平台执行不同的注册逻辑
      if (selectedPlatform === 'kiro') {
        // Kiro 平台：使用 AWS Builder ID 自动注册
        const result: AutoRegisterResult = await autoRegisterKiro({
          email: account.email,
          emailPassword: account.password,
          firstName: account.firstName,
          lastName: account.lastName,
          proxyUrl: PROXY_URL,
        });

        if (result.success && result.ssoToken) {
          updateAccountStatus(account.id, {
            status: 'success',
            ssoToken: result.ssoToken,
            awsName: result.name,
          });
          addLog(`[${account.email}] ✓ ${t('autoRegister.success')}`);

          // 使用 SSO Token 导入账号到系统
          await importWithSsoToken(account, result.ssoToken);
        } else {
          updateAccountStatus(account.id, {
            status: 'failed',
            error: result.error || '注册失败',
          });
          addLog(`[${account.email}] ✗ ${t('autoRegister.failed')}: ${result.error}`);
        }
      } else if (selectedPlatform === 'windsurf') {
        const fullName = `${account.firstName} ${account.lastName}`.trim();
        const browserPath = selectedBrowser === 'custom' ? customBrowserPath : selectedBrowser;

        // 半自动模式：显示信息供用户复制
        if (browserPath === 'semi-auto') {
          updateAccountStatus(account.id, { status: 'registering' });
          setWindsurfInfo({
            email: account.email,
            firstName: account.firstName,
            lastName: account.lastName,
            password: account.password,
            registerUrl: 'https://www.windsurf.com/windsurf/signup',
          });
          setShowWindsurfInfo(true);
          addLog(`[${account.email}] 请手动填写注册信息`);

          // 等待用户完成（通过状态变化来判断）
          await new Promise<void>((resolve) => {
            const checkInterval = setInterval(() => {
              const currentAccount = useAutoRegisterStore.getState().accounts.find(a => a.id === account.id);
              if (currentAccount?.status === 'success' || useAutoRegisterStore.getState().shouldStop) {
                clearInterval(checkInterval);
                setShowWindsurfInfo(false);
                resolve();
              }
            }, 1000);
          });
          return;
        }

        // 其他模式：打开浏览器进行 OAuth 授权
        const result = await autoRegisterWindsurf({
          email: account.email,
          name: fullName,
          proxyUrl: PROXY_URL,
          browserPath,
        });

        if (result.success && result.data) {
          updateAccountStatus(account.id, {
            status: 'success',
            accessToken: result.data.accessToken,
          });
          addLog(`[${account.email}] ✓ Windsurf 授权成功`);
          addLog(`[${account.email}] ✓ 已添加到账号管理器`);
        } else {
          updateAccountStatus(account.id, {
            status: 'failed',
            error: result.error || '授权失败',
          });
          addLog(`[${account.email}] ✗ ${t('autoRegister.failed')}: ${result.error}`);
        }
      } else {
        // 其他平台：暂不支持自动注册
        updateAccountStatus(account.id, {
          status: 'failed',
          error: `平台 ${selectedPlatform} 暂不支持自动注册`,
        });
        addLog(`[${account.email}] ✗ 平台 ${selectedPlatform} 暂不支持自动注册`);
      }
    } catch (error) {
      updateAccountStatus(account.id, {
        status: 'failed',
        error: String(error),
      });
      addLog(`[${account.email}] ✗ ${t('autoRegister.error')}: ${error}`);
    }
  };

  // 使用 SSO Token 导入账号到系统
  const importWithSsoToken = async (
    account: RegisterAccount,
    ssoToken: string
  ): Promise<void> => {
    try {
      addLog(`[${account.email}] 正在导入账号...`);

      const fullName = `${account.firstName} ${account.lastName}`.trim();
      const result = await importFromSsoToken(ssoToken, 'us-east-1', account.email, fullName);

      if (result.success && result.data) {
        // 这里可以调用添加账号到系统的方法
        // 例如：await addKiroAccount(result.data);
        addLog(`[${account.email}] ✓ 已添加到账号管理器`);
      } else {
        addLog(`[${account.email}] ⚠ SSO Token 导入失败: ${result.error}`);
      }
    } catch (error) {
      addLog(`[${account.email}] ✗ 导入出错: ${error}`);
    }
  };

  const startRegistration = async () => {
    const newEmail = generateRandomEmail();
    const parsed = parseAccounts(newEmail);
    if (parsed.length === 0) return;

    addAccounts(parsed);
    addLog(`[${newEmail}] 已添加，开始注册...`);

    const pendingAccounts = parsed.filter((a) => a.status === 'pending' || a.status === 'failed');
    if (pendingAccounts.length === 0) return;

    setIsRunning(true);
    resetStop();
    addLog(`========== ${t('autoRegister.startBatch')} ==========`);
    addLog(`${t('autoRegister.pending')}: ${pendingAccounts.length}`);
    addLog(`平台: ${selectedPlatform}`);

    // 串行处理注册（并发=1）
    for (const account of pendingAccounts) {
      if (useAutoRegisterStore.getState().shouldStop) {
        addLog(t('autoRegister.userStopped'));
        break;
      }
      await registerSingleAccount(account);
    }

    setIsRunning(false);
    const stats = getStats();
    addLog(`========== ${t('autoRegister.completed')} ==========`);
    addLog(`${t('autoRegister.success')}: ${stats.success}, ${t('autoRegister.failed')}: ${stats.failed}`);
  };

  const stopRegistration = async () => {
    requestStop();
    addLog(t('autoRegister.stopping'));
    // 调用后端终止进程
    try {
      const stopped = await stopAutoRegister();
      if (stopped) {
        addLog(t('autoRegister.userStopped'));
      }
    } catch (error) {
      addLog(`停止注册失败: ${error}`);
    }
  };

  const getStatusBadge = (status: RegisterAccount['status']) => {
    switch (status) {
      case 'pending':
        return (
          <span className="status-badge status-pending">
            <Clock size={12} />
            {t('autoRegister.status.pending')}
          </span>
        );
      case 'exists':
        return (
          <span className="status-badge status-exists">
            <AlertCircle size={12} />
            {t('autoRegister.status.exists')}
          </span>
        );
      case 'registering':
        return (
          <span className="status-badge status-registering">
            <Loader2 size={12} className="animate-spin" />
            {t('autoRegister.status.registering')}
          </span>
        );
      case 'getting_code':
        return (
          <span className="status-badge status-getting-code">
            <Mail size={12} />
            {t('autoRegister.status.gettingCode')}
          </span>
        );
      case 'success':
        return (
          <span className="status-badge status-success">
            <CheckCircle size={12} />
            {t('autoRegister.status.success')}
          </span>
        );
      case 'failed':
        return (
          <span className="status-badge status-failed">
            <XCircle size={12} />
            {t('autoRegister.status.failed')}
          </span>
        );
    }
  };

  const stats = getStats();

  return (
    <main className="main-content auto-register-page">
      {/* 页面头部 */}
      <div className="page-heading auto-register-heading">
        <div className="page-title-section">
          <h1>
            <Mail size={22} />
            {t('autoRegister.pageTitle')}
          </h1>
          <p className="page-subtitle">{t('autoRegister.pageSubtitle')}</p>
        </div>
        <div className="page-actions">
          {!isRunning && (
            <>
              <select
                value={selectedPlatform}
                onChange={(e) => setSelectedPlatform(e.target.value)}
                className="platform-select"
                disabled={isRunning}
              >
                {PLATFORM_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              
              {/* 默认邮箱域名配置 */}
              <div className="email-domain-config">
                <span className="email-domain-label">@</span>
                <input
                  type="text"
                  value={defaultEmailDomain}
                  onChange={(e) => saveDefaultEmailDomain(e.target.value)}
                  placeholder="邮箱域名"
                  className="email-domain-input"
                  disabled={isRunning}
                  title="默认邮箱域名，自动生成邮箱时使用"
                />
              </div>
              
              {/* 浏览器选择 - 对所有平台开放 */}
              <select
                value={selectedBrowser}
                onChange={(e) => {
                  const value = e.target.value;
                  setSelectedBrowser(value);
                  setShowCustomBrowserInput(value === 'custom');
                }}
                className="browser-select"
                disabled={isRunning}
                title="选择浏览器"
              >
                {BROWSER_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              
              {showCustomBrowserInput && (
                <input
                  type="text"
                  value={customBrowserPath}
                  onChange={(e) => setCustomBrowserPath(e.target.value)}
                  placeholder="输入浏览器可执行文件路径..."
                  className="custom-browser-input"
                  disabled={isRunning}
                />
              )}
              
              <button
                onClick={handleClear}
                disabled={accounts.length === 0}
                className="btn btn-secondary"
              >
                <Trash2 size={16} />
                {t('common.clear')}
              </button>
            </>
          )}
          {isRunning ? (
            <button onClick={stopRegistration} className="btn btn-danger">
              <Square size={16} />
              {t('common.stop')}
            </button>
          ) : (
            <button
              onClick={startRegistration}
              className="btn btn-primary"
            >
              <Play size={16} />
              {t('autoRegister.startRegistration')}
            </button>
          )}
        </div>
      </div>

      {/* 统计信息 */}
      {accounts.length > 0 && (
        <div className="stats-grid">
          <div className="stat-card">
            <div className="stat-value">{stats.total}</div>
            <div className="stat-label">{t('autoRegister.stats.total')}</div>
          </div>
          <div className="stat-card stat-success">
            <div className="stat-value">{stats.success}</div>
            <div className="stat-label">{t('autoRegister.stats.success')}</div>
          </div>
          <div className="stat-card stat-failed">
            <div className="stat-value">{stats.failed}</div>
            <div className="stat-label">{t('autoRegister.stats.failed')}</div>
          </div>
          <div className="stat-card stat-exists">
            <div className="stat-value">{stats.exists}</div>
            <div className="stat-label">{t('autoRegister.stats.exists')}</div>
          </div>
        </div>
      )}

      {/* Windsurf 半自动模式信息卡片 */}
      {showWindsurfInfo && windsurfInfo && (
        <div className="card windsurf-info-card">
          <div className="card-header">
            <div className="card-title">
              <Mail size={18} />
              Windsurf 注册信息 - 请手动填写
            </div>
          </div>
          <div className="card-content">
            <div className="windsurf-info-content">
              <p className="windsurf-info-hint">
                请打开 <a href={windsurfInfo.registerUrl} target="_blank" rel="noopener noreferrer">Windsurf 注册页面</a> 并填写以下信息：
              </p>

              <div className="windsurf-info-grid">
                <div className="windsurf-info-item">
                  <label>Email</label>
                  <div className="windsurf-info-value">
                    <code>{windsurfInfo.email}</code>
                    <button
                      onClick={() => copyToClipboard(windsurfInfo.email, 'Email')}
                      className="btn btn-sm btn-secondary"
                    >
                      复制
                    </button>
                  </div>
                </div>

                <div className="windsurf-info-item">
                  <label>First Name</label>
                  <div className="windsurf-info-value">
                    <code>{windsurfInfo.firstName}</code>
                    <button
                      onClick={() => copyToClipboard(windsurfInfo.firstName, 'First Name')}
                      className="btn btn-sm btn-secondary"
                    >
                      复制
                    </button>
                  </div>
                </div>

                <div className="windsurf-info-item">
                  <label>Last Name</label>
                  <div className="windsurf-info-value">
                    <code>{windsurfInfo.lastName}</code>
                    <button
                      onClick={() => copyToClipboard(windsurfInfo.lastName, 'Last Name')}
                      className="btn btn-sm btn-secondary"
                    >
                      复制
                    </button>
                  </div>
                </div>

                <div className="windsurf-info-item">
                  <label>Password</label>
                  <div className="windsurf-info-value">
                    <code>{windsurfInfo.password}</code>
                    <button
                      onClick={() => copyToClipboard(windsurfInfo.password, 'Password')}
                      className="btn btn-sm btn-secondary"
                    >
                      复制
                    </button>
                  </div>
                </div>
              </div>

              <div className="windsurf-info-actions">
                <button
                  onClick={openWindsurfRegister}
                  className="btn btn-primary"
                >
                  <Mail size={16} />
                  打开注册页面
                </button>
                <button
                  onClick={() => {
                    // 标记为成功（用户手动完成）
                    const account = accounts.find(a => a.email === windsurfInfo.email);
                    if (account) {
                      updateAccountStatus(account.id, { status: 'success' });
                      addLog(`[${windsurfInfo.email}] ✓ 用户确认注册完成`);
                    }
                    setShowWindsurfInfo(false);
                  }}
                  className="btn btn-success"
                >
                  <CheckCircle size={16} />
                  我已完成注册
                </button>
                <button
                  onClick={() => setShowWindsurfInfo(false)}
                  className="btn btn-secondary"
                >
                  <XCircle size={16} />
                  取消
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 验证码输入框 - 当需要时显示 */}
      {showCodeInput && pendingEmail && (
        <div className="card verification-code-card">
          <div className="card-header">
            <div className="card-title">
              <Key size={18} />
              输入验证码
            </div>
          </div>
          <div className="card-content">
            <div className="verification-code-content">
              <p className="verification-code-hint">
                请查看邮箱 <strong>{pendingEmail}</strong> 获取 AWS 验证码，并输入6位数字：
              </p>
              <div className="verification-code-input-group">
                <input
                  ref={codeInputRef}
                  type="text"
                  maxLength={6}
                  value={verificationCode}
                  onChange={(e) => {
                    const value = e.target.value.replace(/\D/g, '').slice(0, 6);
                    setVerificationCode(value);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && verificationCode.length === 6) {
                      submitVerificationCode();
                    }
                  }}
                  placeholder="000000"
                  className="verification-code-input"
                />
                <button
                  onClick={submitVerificationCode}
                  disabled={verificationCode.length !== 6}
                  className="btn btn-primary"
                >
                  <CheckCircle size={16} />
                  提交
                </button>
                <button
                  onClick={cancelVerificationCode}
                  className="btn btn-secondary"
                >
                  <XCircle size={16} />
                  取消
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 日志区域 */}
      <div className="card">
        <div className="card-header card-header-flex">
          <div className="card-title">
            <Terminal size={18} />
            {t('autoRegister.logs')}
          </div>
          <button onClick={clearLogs} className="btn btn-ghost btn-sm">
            <Trash2 size={14} />
          </button>
        </div>
        <div className="card-content">
          <div className="logs-container">
            {logs.length === 0 ? (
              <div className="logs-empty">{t('autoRegister.noLogs')}</div>
            ) : (
              logs.map((log, i) => (
                <div
                  key={i}
                  className={`log-line ${
                    log.includes('✓')
                      ? 'log-success'
                      : log.includes('✗') || log.includes(t('autoRegister.error')) || log.includes(t('autoRegister.failed'))
                        ? 'log-error'
                        : log.includes('=====')
                          ? 'log-highlight'
                          : 'log-normal'
                  }`}
                >
                  {log}
                </div>
              ))
            )}
            <div ref={logEndRef} />
          </div>
        </div>
      </div>

      {/* 账号列表 */}
      {accounts.length > 0 && (
        <div className="card accounts-card">
          <div className="card-header">
            <div className="card-title">
              <Key size={18} />
              {t('autoRegister.registrationList')}
            </div>
          </div>
          <div className="card-content">
            <div className="accounts-table-container">
              <table className="accounts-table">
                <thead>
                  <tr>
                    <th>{t('autoRegister.table.no')}</th>
                    <th>{t('autoRegister.table.email')}</th>
                    <th>{t('autoRegister.table.name')}</th>
                    <th>{t('autoRegister.table.status')}</th>
                  </tr>
                </thead>
                <tbody>
                  {accounts.map((account, index) => (
                    <tr key={account.id}>
                      <td>{index + 1}</td>
                      <td className="email-cell">{account.email}</td>
                      <td>{`${account.firstName} ${account.lastName}`}</td>
                      <td>{getStatusBadge(account.status)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* 使用说明 */}
      {/* <div className="card help-card">
        <div className="card-header">
          <div className="card-title">
            <AlertCircle size={18} />
            {t('autoRegister.instructions')}
          </div>
        </div>
        <div className="card-content">
          <div className="help-content">
            <p>1. <strong>自动生成</strong>: 点击"开始注册"后系统自动生成随机邮箱并完成注册</p>
            <p>2. <strong>账号信息</strong>:</p>
            <p className="help-indent">- 邮箱: 随机生成 @{defaultEmailDomain} 邮箱（可配置）</p>
            <p className="help-indent">- 密码: 统一使用 admin123456aA!</p>
            <p className="help-indent">- 姓名: 随机从常用英文名库中选择</p>
            <p>3. <strong>代理</strong>: 固定使用 http://127.0.0.1:7890</p>
            <p className="help-warning">
              <AlertCircle size={14} />
              {t('autoRegister.help.browserNote')}: npx playwright install chromium
            </p>
          </div>
        </div>
      </div> */}
    </main>
  );
}

