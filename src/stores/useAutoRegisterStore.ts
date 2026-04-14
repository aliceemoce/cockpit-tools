import { create } from 'zustand';

export interface RegisterAccount {
  id: string;
  email: string;
  password: string;
  firstName: string;
  lastName: string;
  status: 'pending' | 'registering' | 'getting_code' | 'success' | 'failed' | 'exists';
  awsName?: string;
  ssoToken?: string;
  accessToken?: string;
  error?: string;
}

interface AutoRegisterState {
  accounts: RegisterAccount[];
  isRunning: boolean;
  logs: string[];
  shouldStop: boolean;
}

interface AutoRegisterActions {
  addAccounts: (accounts: RegisterAccount[]) => void;
  clearAccounts: () => void;
  updateAccountStatus: (id: string, updates: Partial<RegisterAccount>) => void;
  addLog: (message: string) => void;
  clearLogs: () => void;
  setIsRunning: (running: boolean) => void;
  requestStop: () => void;
  resetStop: () => void;
  getStats: () => {
    total: number;
    success: number;
    failed: number;
    exists: number;
  };
}

type AutoRegisterStore = AutoRegisterState & AutoRegisterActions;

export const useAutoRegisterStore = create<AutoRegisterStore>()((set, get) => ({
  accounts: [],
  isRunning: false,
  logs: [],
  shouldStop: false,

  addAccounts: (newAccounts) => {
    set((state) => ({
      accounts: [...state.accounts, ...newAccounts]
    }));
  },

  clearAccounts: () => {
    if (get().isRunning) return;
    set({ accounts: [], logs: [] });
  },

  updateAccountStatus: (id, updates) => {
    set((state) => ({
      accounts: state.accounts.map((acc) =>
        acc.id === id ? { ...acc, ...updates } : acc
      )
    }));
  },

  addLog: (message) => {
    const timestamp = new Date().toLocaleTimeString();
    set((state) => ({
      logs: [...state.logs, `[${timestamp}] ${message}`]
    }));
  },

  clearLogs: () => {
    set({ logs: [] });
  },

  setIsRunning: (running) => {
    set({ isRunning: running });
  },

  requestStop: () => {
    set({ shouldStop: true });
  },

  resetStop: () => {
    set({ shouldStop: false });
  },

  getStats: () => {
    const accounts = get().accounts;
    return {
      total: accounts.length,
      success: accounts.filter((a) => a.status === 'success').length,
      failed: accounts.filter((a) => a.status === 'failed').length,
      exists: accounts.filter((a) => a.status === 'exists').length
    };
  }
}));
