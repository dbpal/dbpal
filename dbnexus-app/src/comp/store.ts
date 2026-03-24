import { create } from "zustand";

export const confirmMsgStore = create<{
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
