import { ReactElement, ReactNode } from "react";
import { create } from "zustand";
import { extractConfigNameFromUuid, LLMConfig, TabContent } from "./helper";
import { AlertColor } from "@mui/material";
import React from "react";
import { persist } from "zustand/middleware";

export const useConfirmMsg = create<{
  confirmMsgs: Map<string, string>; // 明确 Map 的键值类型
  addConfirmMsg: (id: string, msg: string) => void;
  removeConfirmMsg: (id: string) => void;
}>((set) => ({
  // 状态：你可以直接用 Map，Zustand 没那么多限制
  confirmMsgs: new Map(),

  // 方法：直接修改逻辑
  addConfirmMsg: (id, msg) =>
    set((state) => {
      const newMap = new Map(state.confirmMsgs);
      newMap.set(id, msg);
      return { confirmMsgs: newMap };
    }),

  removeConfirmMsg: (id) =>
    set((state) => {
      const newMap = new Map(state.confirmMsgs);
      newMap.delete(id);
      return { confirmMsgs: newMap };
    }),
}));

interface AppConfigState {
  maxTree: number;
  matchPattern: string[];
  setMaxTree: (val: number) => void;
  setMatchPattern: (matchPattern: string[]) => void;
}

// Move the <AppConfigState> generic here inside the persist wrapper
export const useMatcher = create<AppConfigState>()(
  persist(
    (set) => ({
      maxTree: 120,
      matchPattern: [],
      setMaxTree: (val) => set({ maxTree: val }),
      setMatchPattern: (xs) => set({ matchPattern: xs }),
    }),
    {
      name: "matcher-storage",
    },
  ),
);
interface BoolSwitchState {
  // 使用 Record 代替 Map
  switches: Record<string, boolean>;
  setSwitch: (key: string, value: boolean) => void;
}

export const useBoolSwitch = create<BoolSwitchState>()(
  persist(
    (set) => ({
      switches: {}, // 初始值是空物件
      setSwitch: (key, value) =>
        set((state) => ({
          switches: {
            ...state.switches,
            [key]: value,
          },
        })),
    }),
    {
      name: "some-bool-switch",
    },
  ),
);

export type ContextMenuState = {
  event: React.MouseEvent<any>;
  content: ReactNode;
};

export const useContextMenu = create<{
  state: ContextMenuState;
  setUpMenu: (state: ContextMenuState) => void;
}>((set) => ({
  state: null,
  setUpMenu: (s: ContextMenuState) => {
    set({ state: s });
  },
}));

export const useTabsManage = create<{
  currentTab: number;
  setCurrentTab: (x: number) => void;
  tabs: TabContent[];
  setTabs: (tabs: TabContent[]) => void;
  appendTab: (tab: TabContent) => void;
}>((set) => ({
  currentTab: 0,
  setCurrentTab: (x: number) => {
    set({ currentTab: x });
  },
  tabs: [],
  setTabs: (tabs: TabContent[]) => {
    set({ tabs });
  },
  appendTab: (tab: TabContent) => {
    // state 就是当前时刻的最新的状态
    set((state) => {
      setTimeout(() => {
        state.setCurrentTab(state.tabs.length);
      }, 100);
      return {
        tabs: [...state.tabs, tab],
      };
    });
  },
}));

export const useLLMConfig = create<{
  config: LLMConfig | null;
  setConfig: (config: LLMConfig) => void;
}>((set) => ({
  config: null,
  setConfig: (config: LLMConfig) => {
    set({ config });
  },
}));

export type AlertMsg = {
  severity: AlertColor;
  title: string;
  message: ReactElement | string | null | undefined;
};

export const useNotification = create<{
  notifications: AlertMsg[];
  showNotification: (severity: AlertColor, title: string, message?) => void;
  closeMsg: (x: number) => void;
}>((set) => ({
  notifications: [],

  closeMsg: (idx) => {
    set((s) => ({
      notifications: s.notifications.filter((_e, index) => idx != index),
    }));
  },
  showNotification: (severity: AlertColor, title: string, err?) => {
    if (typeof err == "object" && "reason" in err) {
      console.error("detail:", err.detail);
      // this a error from the backend
      // we only show `reason` the short message
      err = err["reason"];
    }
    if (!React.isValidElement(err) && err) {
      err = String(err);
    }
    set((state) => {
      return {
        notifications: [...state.notifications, { severity, title, message: err }],
      };
    });

    setTimeout(() => {
      set((state) => ({ notifications: state.notifications.slice(1) }));
    }, 10000);
  },
}));

export const useConnected = create<{
  connected: Set<string>;
  is_connected: (name: string) => boolean;
  setConnected: (name: string) => void;
  removeConnected: (name: string) => void;
}>((set, get) => ({
  connected: new Set(),

  // Use get() to access the current state
  is_connected: (name) => {
    name = extractConfigNameFromUuid(name);
    return get().connected.has(name);
  },

  setConnected: (name) => {
    set((state) => {
      // Create a new Set instance to ensure React detects the change
      const nextSet = new Set(state.connected);
      nextSet.add(name);
      return { connected: nextSet };
    });
  },
  removeConnected: (name) => {
    set((state) => {
      // Create a new Set instance to ensure React detects the change
      const nextSet = new Set(state.connected);
      nextSet.delete(name);
      return { connected: nextSet };
    });
  },
}));

export const useQueryExtraUI = create<{
  extra: Map<string, ReactNode>;
  setExtra: (id: string, extra: ReactNode) => void;
  removeExtra: (id: string) => void;
}>((set) => ({
  extra: new Map(),

  setExtra: (id, extra) =>
    set((state) => {
      const nextMap = new Map(state.extra);
      nextMap.set(id, extra);
      return { extra: nextMap };
    }),

  removeExtra: (id) =>
    set((state) => {
      const nextMap = new Map(state.extra);
      nextMap.delete(id);
      return { extra: nextMap };
    }),
}));

export const useConnLoading = create<{
  loading: Set<string>;
  setLoading: (name: string, loading: boolean) => void;
  isLoading: (name: string) => boolean;
}>((set, get) => ({
  loading: new Set(),

  isLoading(name) {
    return get().loading.has(name);
  },
  setLoading: (name, loading) =>
    set((state) => {
      const nextSet = new Set(state.loading);
      if (loading) {
        nextSet.add(name);
      } else {
        nextSet.delete(name);
      }
      return { loading: nextSet };
    }),
}));

interface PageSizeState {
  sql: number;
  couchbase: number;
  mongo: number;
  // Actions to update the state
  setPageSize: (db: keyof Omit<PageSizeState, "setPageSize">, size: number) => void;
}

export const usePageSize = create<PageSizeState>((set) => ({
  // Initial State
  sql: 100,
  couchbase: 10,
  mongo: 10,

  // Action to update a specific database's page size
  setPageSize: (db, size) =>
    set((state) => ({
      ...state,
      [db]: size,
    })),
}));

interface SizeState {
  // 核心數據：例如 { "sidebar": 300, "editor": 500 }
  sizes: Record<string, number>;

  // 設定特定 Key 的數值
  setSize: (key: string, value: number) => void;

  // 獲取數值（支援傳入預設值，防止 undefined）
  getSize: (key: string, defaultValue: number) => number;

  // 批量更新或重置
  resetSizes: (newSizes: Record<string, number>) => void;
}

export const useSizeStore = create<SizeState>()(
  persist(
    (set, get) => ({
      sizes: {},
      setSize: (key, value) =>
        set((state) => ({
          sizes: {
            ...state.sizes,
            [key]: value,
          },
        })),

      // 注意：Zustand 的 state 雖然是響應式的，但 getSize 是一個 Helper 函數
      getSize: (key, defaultValue) => {
        return get().sizes[key] ?? defaultValue;
      },

      resetSizes: (sizes) => set({ sizes }),
    }),
    {
      name: "couchbase-ui-sizes",
    },
  ),
);
