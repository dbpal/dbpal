import { KeyboardEvent, ReactNode } from "react";
import { create } from "zustand";

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
  event: KeyboardEvent;
  content: ReactNode;
};

export const useContextMenu = create<{
  state: ContextMenuState;
  setUpMenu: (state: ContextMenuState) => void;
}>((set) => ({
  state: null,
  setUpMenu: (s: ContextMenuState) => {
    set(s);
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
