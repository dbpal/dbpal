import { create } from "zustand";

interface ConfirmState {
  confirmMsgs: Map<string, string>; // 明确 Map 的键值类型
  addConfirmMsg: (id: string, msg: string) => void;
  removeConfirmMsg: (id: string) => void;
}

// 连 Provider 都不用，直接创建一个 Hook！
export const tabHasConfirmMsg = create<ConfirmState>((set) => ({
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
