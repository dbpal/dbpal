import { ReactNode } from "react";
import { create } from "zustand";
import { LLMConfig, TabContent } from "./helper";

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

// export const settingStore = create<{
//   setting: UserSetting | null;
//   setSetting: (setting: UserSetting) => void;
// }>((set) => ({
//   setting: null,
//   setSetting: (setting: UserSetting) =>
//     set(() => ({
//       setting,
//     })),
// }));
