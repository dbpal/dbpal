import { ReactElement, ReactNode } from "react";
import { create } from "zustand";
import { extractConfigNameFromUuid, LLMConfig, TabContent } from "./helper";
import { AlertColor } from "@mui/material";
import React from "react";

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
  // 对应 MaxTreeContext 的值
  maxTree: number;
  // 对应 MatchPatternContext 的函数
  matchPattern: (x: string) => boolean;

  // 更新方法
  setMaxTree: (val: number) => void;
  setMatchPattern: (fn: (x: string) => boolean) => void;
}

export const useAppConfigStore = create<AppConfigState>((set) => ({
  // 初始值（取自你之前的 defaultValue）
  maxTree: 120,
  matchPattern: () => true,

  // 修改状态的方法
  setMaxTree: (val) => set({ maxTree: val }),
  setMatchPattern: (fn) => set({ matchPattern: fn }),
}));

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
