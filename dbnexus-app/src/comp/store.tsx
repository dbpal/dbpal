import { ReactElement, ReactNode, useCallback, useMemo } from "react";
import { create, StateCreator } from "zustand";
import { AlertColor, AlertTitle, IconButton, Stack, Typography } from "@mui/material";
import React from "react";
import { persist, PersistStorage, StorageValue } from "zustand/middleware";
import Dexie, { type Table } from "dexie";
import wcmatch from "wildcard-match";
import { cloneDeep } from "lodash-es";
import { useSnackbar } from "notistack";
import { PassWordManager, uuidv4 } from "./Common";
import { Channel } from "@tauri-apps/api/core";
import { cancelTask } from "./handlers";
import CloseIcon from "@mui/icons-material/Close";
import {
  AiOnlineSearch,
  AISteamResp,
  ChatHistory,
  commands,
  DBConnectionConfig,
  OpenAIReq,
} from "./bindings";
import { LazyStore } from "@tauri-apps/plugin-store";
import { extractConfigNameFromUuid, TabContent } from "./helper";
import { GridColumnVisibilityModel } from "./datagridExport";
import { emit, listen } from "@tauri-apps/api/event";

export function getAppStore() {
  return new LazyStore("store.json");
}

const eventNameReloadStore = "event-reload-store";
class TauriStoreManger<S> implements PersistStorage<S> {
  appStore = getAppStore();
  constructor(
    public setBefore?: (_: S) => Promise<S>,
    public afterFetch?: (_: S) => Promise<void>,
  ) {}
  async setItem(name: string, value: StorageValue<S>) {
    if (this.setBefore) {
      const new_value = cloneDeep(value);
      new_value.state = await this.setBefore(new_value.state);
      await this.appStore.set(name, new_value);
    } else {
      await this.appStore.set(name, value);
    }
    emit(eventNameReloadStore);
  }

  async getItem(name: string) {
    let ret = await this.appStore.get<StorageValue<S>>(name);
    if (this.afterFetch) {
      await this.afterFetch(ret.state);
    }
    return ret;
  }

  removeItem(name: string) {
    this.appStore.delete(name);
  }
}

interface TableColumnVisible {
  // Key format: "tableName:uuid" -> Visibility Model
  configs: Record<string, GridColumnVisibilityModel>;

  // Action to update visibility for a specific table instance
  setColumnVisibility: (
    tableName: string,
    uuid: string,
    model: GridColumnVisibilityModel,
  ) => void;

  // Helper to get visibility (returns empty object if not found)
  getColumnVisibility: (tableName: string, uuid: string) => GridColumnVisibilityModel;
}

export const useTableColumnVisibleStore = create<TableColumnVisible>()(
  persist(
    (set, get) => ({
      configs: {},

      setColumnVisibility: (tableName, uuid, model) => {
        const key = `${tableName}:${uuid}`;
        set((state) => ({
          configs: {
            ...state.configs,
            [key]: model,
          },
        }));
      },

      getColumnVisibility: (tableName, uuid) => {
        const key = `${tableName}:${uuid}`;
        return get().configs[key] || {};
      },
    }),
    {
      name: "mui-datagrid-columns-visible",
    },
  ),
);

export const useConfirmMsg = create<{
  confirmMsgs: Map<string, string>; // 明确 Map 的键值类型
  addConfirmMsg: (id: string, msg: string) => void;
  removeConfirmMsg: (id: string) => void;
  getConfirmMsg: (id: string) => string | undefined;
}>((set, get) => ({
  // 状态：你可以直接用 Map，Zustand 没那么多限制
  confirmMsgs: new Map(),

  getConfirmMsg: (id) => get().confirmMsgs.get(id),
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

interface MatcherTreeState {
  maxTree: number;
  matchPattern: string[];
  setMaxTree: (val: number) => void;
  setMatchPattern: (matchPattern: string[]) => void;
}

// Move the <AppConfigState> generic here inside the persist wrapper
export const useMatcher = create<MatcherTreeState>()(
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
  getSwitch: (key: string) => boolean;
}

export const useBoolSwitch = create<BoolSwitchState>()(
  persist(
    (set, get) => ({
      getSwitch: (key) => get().switches[key] || false,
      switches: {}, // 初始值是空物件
      setSwitch: (key, value: boolean) =>
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
  closeTab: (index: number) => void;
}>((set) => ({
  currentTab: 0,
  setCurrentTab: (x: number) => {
    set({ currentTab: x });
  },
  closeTab: (index: number) => {
    set((state) => {
      const newTabs = [...state.tabs];
      newTabs.splice(index, 1);
      return {
        tabs: newTabs,
      };
    });
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

type LLMConfig = {
  baseUrl: string;
  apiKey: string;
  model: string;
  temperature: number;
  maxToken: number;
  topP: number;
  search_engin: AiOnlineSearch;
};

const LLM_CONFIG_KEY = "llm-config-4k9zF7m5p3T1H2";

interface LLMStore {
  config: LLMConfig;
  setConfig: (config: LLMConfig) => void;
}

export const useLLMConfig = create<LLMStore>()(
  persist(
    (set) => ({
      config: {
        baseUrl: "https://api.openai.com/v1",
        apiKey: "",
        maxToken: 0,
        model: "gpt-3.5-turbo",
        temperature: null,
        topP: 0,
        search_engin: {
          enable: true,
          provider: "duckduckgo",
          api_key: "",
        },
      },
      setConfig: (config) => set({ config }),
    }),

    {
      name: "llm-config",
      version: 1,
      storage: new TauriStoreManger<LLMStore>(
        async (s) => {
          if (s.config.apiKey) {
            await PassWordManager.savePassword(
              PassWordManager.serviceLLM,
              LLM_CONFIG_KEY,
              s.config.apiKey,
              true,
            );
          } else {
            await PassWordManager.deletePassword(
              PassWordManager.serviceLLM,
              LLM_CONFIG_KEY,
            );
          }
          return { ...s, apiKey: "" };
        },
        async (s) => {
          let pass = await PassWordManager.getPassword(
            PassWordManager.serviceLLM,
            LLM_CONFIG_KEY,
          );
          s.config.apiKey = pass;
        },
      ),
    },
  ),
);

export const makeLLMCallable = (config: LLMConfig) => {
  const makeReq = (
    prompt: string,
    images,
    documents: string[],
    history: ChatHistory[] = [],
    user_system_prompt: string | null,
  ): OpenAIReq => {
    return {
      url: config.baseUrl,
      api_key: config.apiKey,
      model: config.model,
      prompt,
      temperature: config.temperature,
      max_tokens: config.maxToken,
      top_p: config.topP,
      images,
      search_engine: config.search_engin,
      chat_history: history,
      user_system_prompt,
      documents,
    };
  };

  return {
    prompt: async (msg: string): Promise<string> => {
      return await commands.openaiInvoke(makeReq(msg, [], [], [], ""));
    },
    stream_prompt: async (
      uuid: string,
      queryId: string,
      prompt: string,
      images: string[],
      documents: string[],
      history: ChatHistory[],
      user_system_prompt: string | null,
      handleMessage: (event: AISteamResp) => void,
      handleCancel: (cancel: () => void) => void,
    ) => {
      console.log(" history:{}", history);
      const taskId = uuidv4();
      const emit = new Channel<AISteamResp>((event) => {
        handleMessage(event);
      });
      handleCancel(() => {
        cancelTask(taskId);
      });
      await commands.openaiStream(
        makeReq(prompt, images, documents, history, user_system_prompt),
        uuid,
        queryId,
        emit,
        taskId,
      );
    },
  };
};

export const useLLM = () => {
  const config = useLLMConfig((s) => s.config);
  const r = useMemo(() => {
    if (config.baseUrl && config.model) {
      return makeLLMCallable(config);
    } else {
      return null;
    }
  }, [config]);

  return r;
};

export type NotificationParams = {
  severity: AlertColor;
  title?: string;
  message?: any;
};

export const useNotification = () => {
  const { enqueueSnackbar, closeSnackbar } = useSnackbar();

  const showNotification = useCallback(
    (params: NotificationParams) => {
      let { severity, title, message } = params;
      if (typeof message == "object" && "reason" in message) {
        console.error("detail:", message.detail);
        // this a error from the backend
        // we only show `reason` the short message
        message = message["reason"];
      }
      if (!React.isValidElement(message) && message) {
        message = String(message);
      }
      enqueueSnackbar(
        <Stack>
          {title && <AlertTitle> {title} </AlertTitle>}
          <div>{message}</div>
        </Stack>,
        {
          variant: severity as "default" | "error" | "success" | "warning" | "info",
          autoHideDuration: 10000,
          anchorOrigin: {
            vertical: "top",
            horizontal: "right",
          },
          action: (snackbarId) => (
            <IconButton onClick={() => closeSnackbar(snackbarId)}>
              <CloseIcon />
            </IconButton>
          ),
        },
      );
    },
    [closeSnackbar, enqueueSnackbar],
  );
  return {
    showNotification,
  };
};

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
  setPageSize: (db: keyof Omit<PageSizeState, "setPageSize">, size: number) => void;
}

export const usePageSize = create<PageSizeState>()(
  persist(
    (set) => ({
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
    }),
    {
      name: "page-size-storage", // 存储在 localStorage 中的 key 名称
      // storage: createJSONStorage(() => localStorage), // 默认就是 localStorage，可省略
    },
  ),
);

interface SizeState {
  // 核心數據：例如 { "sidebar": 300, "editor": 500 }
  sizes: Record<string, number>;

  // 設定特定 Key 的數值
  setSize: (key: string, value: number) => void;

  // 獲取數值（支援傳入預設值，防止 undefined）
  getSize: (x: UseSizeConfig) => number;

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
      getSize: (x: UseSizeConfig) => {
        return get().sizes[x.key] ?? x.default;
      },
      resetSizes: (sizes) => set({ sizes }),
    }),
    {
      name: "some-sizes",
    },
  ),
);

export class UseSizeConfig {
  key: string;
  min: number;
  max: number;
  step?: number;
  default: number;
  name: string;

  notice: ReactNode;

  static chatMemoryKey: UseSizeConfig = {
    key: "chat-memory-k",
    min: 0,
    default: 6,
    max: 20,
    step: 2,
    notice: "chat memory buffer size",
    name: "memory size",
  };

  static gridCol: UseSizeConfig = {
    key: "grid-col",
    min: 1,
    max: 6,
    default: 1,
    notice: "grid column count",
    name: "column",
  };

  static editorHeight: UseSizeConfig = {
    key: "editor-heigh",
    min: 300,
    max: 1200,
    step: 10,
    default: 300,
    notice: "editor height",
    name: "editor height",
  };

  static queryLimit: UseSizeConfig = {
    key: "query-limit",
    min: 1000,
    max: 10_00_00,
    step: 1000,
    default: 10000,
    name: "query limit",
    notice: (
      <Typography>
        limit max rows can be query.we don&apos;t want OOM!!!😀
        <br />
        dbpal use Stream technology to query data,so we can query large data
      </Typography>
    ),
  };
}

// 1. Define the shape of our state
interface ColumnWidthsState {
  // Key is the column field (string), value is the width (number)
  columnWidths: Record<string, number>;

  // Action to update a specific column
  setColumnWidth: (field: string, width: number) => void;

  // Optional: Reset all widths
  resetWidths: () => void;

  //

  getColumnWidth: (field: string) => number | undefined;
}

export const useColumnWidthStore = create<ColumnWidthsState>()(
  persist(
    (set, get) => ({
      columnWidths: {},

      getColumnWidth(field) {
        return get().columnWidths[field];
      },

      setColumnWidth: (field, width) =>
        set((state) => ({
          columnWidths: {
            ...state.columnWidths,
            [field]: width,
          },
        })),

      resetWidths: () => set({ columnWidths: {} }),
    }),
    {
      name: "column-width-storage",
    },
  ),
);

interface TableFilterState {
  // Key: "uuid-name" | Value: Unique array of strings
  filters: Record<string, string[]>;

  // Actions
  addFilter: (uuid: string, name: string, filterValue: string) => void;
  removeFilter: (uuid: string, name: string) => void;
  // Add the definition here
  getFilter: (uuid: string, name: string) => string[];
}

// Defining the creator separately helps TS resolve the 'Partial' conflict
const storeApi: StateCreator<TableFilterState, [["zustand/persist", unknown]]> = (
  set,
  get,
) => ({
  filters: {},

  addFilter: (uuid, name, filterValue) => {
    const key = `${uuid}-${name}`;
    set((state) => {
      const currentList = state.filters[key] || [];
      if (currentList.includes(filterValue)) return state;

      return {
        filters: {
          ...state.filters,
          [key]: [...currentList, filterValue],
        },
      };
    });
  },

  getFilter: (uuid, name) => {
    const key = `${uuid}-${name}`;
    // get() now correctly recognizes the state structure
    return get().filters[key] || [];
  },

  removeFilter: (uuid, name) => {
    const key = `${uuid}-${name}`;
    set((state) => {
      const newFilters = { ...state.filters };
      delete newFilters[key];
      return { filters: newFilters };
    });
  },
});

export const useTableFilter = create<TableFilterState>()(
  persist(storeApi, {
    name: "table-filter-storage",
  }),
);

// 1. 定义数据项接口 (对应 SQL 表列)
interface ElementSize {
  width: number | null;
  height: number | null;
}

// 2. 定义 Store 的状态与操作接口
interface ElementSizeState {
  // 使用对象存储，Key 为 name，Value 为数据项，模拟主键索引
  elements: Record<string, ElementSize>;

  // 操作方法：增删改
  setElement: (name, element: ElementSize) => void;
}

// 3. 创建支持持久化的 Store
export const useElementSizeStore = create<ElementSizeState>()(
  persist(
    (set) => ({
      elements: {}, // 初始状态

      // 实现设置/更新逻辑 (模拟 INSERT OR REPLACE)
      setElement: (name, element) =>
        set((state) => ({
          elements: {
            ...state.elements,
            [name]: element,
          },
        })),

      // 实现删除逻辑 (模拟 DELETE)
    }),
    {
      name: "element-size-storage",
    },
  ),
);

export interface UserSettings {
  appearance: {
    fontSize: number;
    fontWeight: string;
    fontFamily: string;
    lightBgColor: string;
    darkBgColor: string;
    datagridRowHeightFactor: number;
    theme: string;
    zoom: number;
  };
  editor: {
    theme: string;
    editorOption: string;
  };
  misc: {
    autoStart: boolean;
    logLevel: string;
  };
  proxy: {
    http: string | "";
    https: string | "";
    no: string | "";
    all: string | "";
  };
}

// 定义 Store 的状态和方法
interface UserSettingsState extends UserSettings {
  // Actions
  updateAppearance: (data: Partial<UserSettings["appearance"]>) => void;

  updateEditor: (data: Partial<UserSettings["editor"]>) => void;
  updateMisc: (data: Partial<UserSettings["misc"]>) => void;

  updateProxy: (data: Partial<UserSettings["proxy"]>) => void;

  // 计算属性 (由 Selector 处理，或在此定义辅助函数)
  getDatagridRowHeight: () => number;
}

const initialSettings: UserSettings = {
  appearance: {
    fontSize: 15,
    fontWeight: "bolder",
    fontFamily: "PT Mono",
    lightBgColor: "",
    darkBgColor: "",
    datagridRowHeightFactor: 2.5,
    theme: "light",
    zoom: 1,
  },
  editor: {
    theme: "",
    editorOption: "{\n}",
  },
  misc: {
    autoStart: false,
    logLevel: "info",
  },
  proxy: {
    http: "",
    https: "",
    no: "",
    all: "",
  },
};

const userSettingKey = "dbpal-combined-settings";

function reloadStore() {
  async function reload() {
    try {
      await useUserSettings.persist.rehydrate();
      await useColumnWidthStore.persist.rehydrate();
      await useBoolSwitch.persist.rehydrate();
      await useLLMConfig.persist.rehydrate();
      await usePageSize.persist.rehydrate();
      await useSizeStore.persist.rehydrate();
      await useColumnWidthStore.persist.rehydrate();
      await useElementSizeStore.persist.rehydrate();
      await useUserSystemPromptStore.persist.rehydrate();
      await useViewInModalMatchStrStore.persist.rehydrate();
      await useExportConfigStore.persist.rehydrate();
    } catch (error) {
      console.error("Error reloading store:", error);
    }
  }
  window.addEventListener("storage", async () => {
    await reload();
  });

  listen(eventNameReloadStore, () => {
    reload();
  });
}
reloadStore();

export const useUserSettings = create<UserSettingsState>()(
  persist(
    (set, get) => ({
      ...initialSettings,

      updateProxy: (data) => set((state) => ({ proxy: { ...state.proxy, ...data } })),

      // 更新逻辑
      updateAppearance: (data) =>
        set((state) => ({ appearance: { ...state.appearance, ...data } })),

      updateEditor: (data) =>
        set((state) => ({ editor: { ...state.editor, ...data } })),

      updateMisc: (data) => set((state) => ({ misc: { ...state.misc, ...data } })),

      // 替代原有的 datagridRowHeight() 方法
      getDatagridRowHeight: () => {
        const { fontSize, datagridRowHeightFactor } = get().appearance;
        return Math.ceil(fontSize * datagridRowHeightFactor);
      },
    }),
    {
      name: userSettingKey, // 建议合并成一个 key，方便管理
      version: 1,
    },
  ),
);

export const useDatagridHeight = () => {
  return useUserSettings((state) =>
    Math.ceil(state.appearance.fontSize * state.appearance.datagridRowHeightFactor),
  );
};

export const useMatcherFn = () => {
  // 1. 只获取数据（只有当数据变了，这里才会重绘）
  const matchPattern = useMatcher((s) => s.matchPattern);

  // 2. 在外部定义逻辑函数
  const matcher = useCallback(
    (name: string) => {
      if (matchPattern.length === 0) {
        return true;
      }
      for (const n of matchPattern) {
        // 优化：wcmatch 可以在循环外或者通过 memo 优化，这里为了逻辑一致保持原样
        const matchFn = wcmatch("*" + n + "*", { flags: "i" });
        if (matchFn(name)) {
          return true;
        }
      }
      return false;
    },
    [matchPattern],
  ); // 只有 pattern 变了，matcher 函数地址才会变

  return matcher;
};

interface ConnectionState {
  connections: DBConnectionConfig[];

  addConnection: (config: DBConnectionConfig) => Promise<void>;
  removeConnection: (name: string) => void;
  nameExists: (name: string) => boolean;
  getConnection: (name: string) => Promise<DBConnectionConfig | null>;
}

export const passwordKey = "password";

export const savePasswordKey = "save_password";

function trySavePassToKeyRing(x: object) {
  return x[savePasswordKey] === true;
}
export const useConnectionStore = create<ConnectionState>()(
  persist(
    (set, get) => ({
      connections: [],

      // Get connection and inject password from Manager
      getConnection: async (name) => {
        const conn = get().connections.find((c) => c.name === name);
        if (!conn) return null;
        // Return a copy with the password injected
        let password = undefined;
        if (savePasswordKey in conn) {
          password = await PassWordManager.getPassword(
            PassWordManager.serviceConnection,
            name,
          );
          if (false === conn[savePasswordKey] && !password) {
            password = window.prompt("please input password:");
          }
        }
        if (password) {
          return { ...conn, password };
        } else {
          return conn;
        }
      },

      // Add or Update
      addConnection: async (config) => {
        // 1. Handle password persistence separately
        if (config[passwordKey]) {
          await PassWordManager.savePassword(
            PassWordManager.serviceConnection,
            config.name,
            config[passwordKey],
            trySavePassToKeyRing(config),
          );
          // Do not store the actual password in the connection state/disk
          config[passwordKey] = "";
        }

        const current = get().connections;
        const exists = current.some((c) => c.name === config.name);
        if (exists) {
          // Update: replace existing item
          set({
            connections: current.map((c) => (c.name === config.name ? config : c)),
          });
        } else {
          // Add: prepend to list
          set({ connections: [config, ...current] });
        }
      },

      // Remove by name
      removeConnection: (name) => {
        PassWordManager.deletePassword(PassWordManager.serviceConnection, name);
        set((state) => ({
          connections: state.connections.filter((c) => c.name !== name),
        }));
        // Note: You might want to call PassWordManager.deletePassword(name) here too
      },

      // Check if name exists in array
      nameExists: (name) => {
        return get().connections.some((c) => c.name === name);
      },
    }),
    {
      name: "connection-storage",
      storage: new TauriStoreManger<ConnectionState>(),
      version: 1,
      onRehydrateStorage: () => {
        console.log("开始同步数据...");
        return (state, error) => {
          if (error) {
            console.error("同步失败:", error);
          } else {
            console.log("同步成功，数据量:", state?.connections?.length);
          }
        };
      },
    },
  ),
);

export interface QueryLog {
  id?: number;
  uuid: string;
  name: string; // connection name
  sql: string;
  create_at: number; // 存储时间戳
  success: number; // 1 或 0
}

interface QueryLogState {
  // 核心操作方法
  save: (uuid: string, sql: string, success: boolean) => Promise<void>;
  query: (uuid: string) => Promise<QueryLog[]>;
  deleteByUuid: (uuid: string) => Promise<number>;
  deleteByName: (name: string) => Promise<void>;
}

const maxQueryLogCount = 10000;

export const useQueryLogStore = create<QueryLogState>(() => ({
  // 保存日志
  save: async (uuid, sql, success) => {
    try {
      // 1. Save the new log
      await db.query_log.add({
        uuid,
        name: extractConfigNameFromUuid(uuid),
        sql,
        success: success ? 1 : 0,
        create_at: Date.now(),
      });

      // 2. Check the count for this specific uuid
      const count = await db.query_log.where("uuid").equals(uuid).count();

      // 3. If over limit, delete the oldest ones
      const LIMIT = maxQueryLogCount;
      if (count > LIMIT) {
        const extraCount = count - LIMIT;

        // Find the IDs of the oldest records for this uuid
        const oldestLogs = await db.query_log
          .where("uuid")
          .equals(uuid)
          .limit(extraCount) // Pick the first 'n' records (oldest)
          .primaryKeys(); // Get just the IDs to speed up deletion

        await db.query_log.bulkDelete(oldestLogs);
        console.info(`Trimmed ${extraCount} old logs for uuid: ${uuid}`);
      }
    } catch (e) {
      console.error("save query log failed:", e);
    }
  },

  query: async (uuid) => {
    let collection = db.query_log
      .where("uuid")
      .equals(uuid)
      .reverse()
      .sortBy("create_at");
    return collection;
  },

  // 删除
  deleteByUuid: async (uuid) => {
    return await db.query_log.where("uuid").equals(uuid).delete();
  },

  deleteByName: async (name) => {
    await db.query_log.where("name").equals(name).delete();
  },
}));

export interface FavoriteSql {
  id?: number;
  uuid: string;
  name: string;
  kind: string;
  sql: string;
  connection_name: string;
  last_modified: number;
}

export class MyDatabase extends Dexie {
  query_log!: Table<QueryLog>;
  favorite_sql!: Table<FavoriteSql>;
  chat_history!: Table<ChatMessage>; // 新增聊天表

  constructor() {
    super("AppDatabase");
    this.version(2).stores({
      // 版本号记得加 1
      query_log: "++id,id,uuid,name,[uuid+create_at]",
      favorite_sql: "++id, uuid, name, connection_name, &[uuid+name]",
      // 新增：给 name 和 timestamp 加索引，方便排序和清理
      chat_history: "++id, name, timestamp",
    });
  }
}

export const db = new MyDatabase();

interface FavoriteStore {
  // Actions
  save: (
    data: Omit<FavoriteSql, "id" | "last_modified" | "connection_name">,
  ) => Promise<void>;
  getByUuid: (uuid?: string) => Promise<FavoriteSql[]>;
  getDistinctUuids: () => Promise<string[]>;
  deleteById: (id: number) => Promise<void>;
  clearByUuid: (uuid: string) => Promise<void>;
  deleteByConnectionName: (name: string) => Promise<void>;
}

export const useFavoriteStore = create<FavoriteStore>(() => ({
  // REPLACE INTO logic using db.put()

  save: async (data) => {
    const LIMIT_PER_UUID = 500; // Favorites usually have a smaller limit than logs
    const connection_name = extractConfigNameFromUuid(data.uuid);

    try {
      // Use a Read/Write transaction for atomicity
      await db.transaction("rw", db.favorite_sql, async () => {
        // 1. Check for existing record (to simulate REPLACE)
        const existing = await db.favorite_sql
          .where("[uuid+name]")
          .equals([data.uuid, data.name])
          .first();

        // 2. Perform the Put (Update if ID exists, otherwise Insert)
        await db.favorite_sql.put({
          ...data,
          id: existing?.id,
          connection_name,
          last_modified: Date.now(),
        });

        // 3. Only check limit if it's a NEW record (existing was undefined)
        if (!existing) {
          const count = await db.favorite_sql.where("uuid").equals(data.uuid).count();

          if (count > LIMIT_PER_UUID) {
            const extraCount = count - LIMIT_PER_UUID;

            // Find the oldest favorites for this uuid
            // We use 'last_modified' or 'id' to determine "oldest"
            const toDelete = await db.favorite_sql
              .where("uuid")
              .equals(data.uuid)
              .limit(extraCount)
              .primaryKeys();

            await db.favorite_sql.bulkDelete(toDelete);
            console.info(
              `Limit reached. Trimmed ${extraCount} favorites for ${data.uuid}`,
            );
          }
        }
      });
    } catch (e) {
      console.error("Save favorite failed:", e);
      throw e;
    }
  },

  getByUuid: async (uuid) => {
    if (!uuid) return await db.favorite_sql.toArray();
    return await db.favorite_sql.where("uuid").equals(uuid).toArray();
  },
  getDistinctUuids: async () => {
    // 先检查总数
    const count = await db.favorite_sql.count();
    if (count === 0) return [];

    // 只有有数据时才执行索引排序
    const uuids = await db.favorite_sql.orderBy("uuid").uniqueKeys();
    return uuids as string[];
  },

  deleteById: async (id) => {
    await db.favorite_sql.delete(id);
  },

  clearByUuid: async (uuid) => {
    await db.favorite_sql.where("uuid").equals(uuid).delete();
  },

  deleteByConnectionName: async (name) => {
    await db.favorite_sql.where("connection_name").equals(name).delete();
  },
}));

export interface ChatMessage {
  id?: number;
  name: string;
  role: "user" | "assistant" | "system"; // 新增：定义角色类型
  content: string;
  timestamp: number;
}

interface ChatState {
  maxLogs: number;
  messagesByNames: Record<string, ChatMessage[]>;
  isLoading: boolean;

  fetchHistory: (name: string) => Promise<ChatMessage[]>;
  // 修改：增加 role 参数
  saveMessage: (name: string, role: ChatMessage["role"], text: string) => Promise<void>;
  clearHistory: (name: string) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messagesByNames: {},
  isLoading: false,
  maxLogs: 100,

  fetchHistory: async (name) => {
    set({ isLoading: true });
    try {
      const history = await db.chat_history
        .where("name")
        .equals(name)
        .sortBy("timestamp");

      set((state) => ({
        messagesByNames: {
          ...state.messagesByNames,
          [name]: history,
        },
        isLoading: false,
      }));

      return history;
    } catch (e) {
      console.error("fetch history failed:", e);
      set({ isLoading: false });
      return [];
    }
  },

  saveMessage: async (name, role, text) => {
    const { maxLogs, fetchHistory } = get();

    try {
      await db.transaction("rw", db.chat_history, async () => {
        await db.chat_history.add({
          name,
          role, // 保存角色信息
          content: text,
          timestamp: Date.now(),
        });

        const count = await db.chat_history.where({ name }).count();
        if (count > maxLogs) {
          const oldestKeys = await db.chat_history
            .where({ name })
            .limit(count - maxLogs)
            .primaryKeys();
          await db.chat_history.bulkDelete(oldestKeys);
        }
      });

      await fetchHistory(name);
    } catch (e) {
      console.error("save message failed:", e);
    }
  },

  clearHistory: async (name) => {
    try {
      await db.chat_history.where({ name }).delete();
      set((state) => {
        const newMessages = { ...state.messagesByNames };
        delete newMessages[name];
        return { messagesByNames: newMessages };
      });
    } catch (e) {
      console.error("clear history failed:", e);
    }
  },
}));

interface UserPromptState {
  prompts: Record<string, string>;
  savePrompt: (name: string, text: string) => void;
  removePrompt: (name: string) => void;
}

export const useUserSystemPromptStore = create<UserPromptState>()(
  persist(
    (set) => ({
      // Initial state
      prompts: {},

      // Save or update a prompt
      savePrompt: (name, text) =>
        set((state) => ({
          prompts: {
            ...state.prompts,
            [name]: text,
          },
        })),

      // Remove a specific prompt
      removePrompt: (name) =>
        set((state) => {
          const newPrompts = { ...state.prompts };
          delete newPrompts[name];
          return { prompts: newPrompts };
        }),
    }),
    {
      name: "user-prompt-storage", // Unique name for the item in storage
      storage: new TauriStoreManger<UserPromptState>(),
    },
  ),
);

interface ViewInModalState {
  matchStr: string;
  setMatchStr: (matchStr: string) => void;
}

export const useViewInModalMatchStrStore = create<ViewInModalState>()(
  persist(
    (set) => ({
      matchStr: "",
      setMatchStr: (matchStr) => set({ matchStr }),
    }),
    {
      name: "view-in-modal-storage-match-str", // 存储在 localStorage 中的唯一 Key
    },
  ),
);

interface ExportConfigState {
  delimiter: string; // 注意：沿用你定义的拼写 "delimiter"
  includeHeader: boolean;

  setDelimiter: (delimiter: string) => void;
  setIncludeHeader: (includeHeader: boolean) => void;

  jsonPretty: boolean;
  setJsonPretty: (jsonPretty: boolean) => void;

  includeTableSchema: boolean;

  setIncludeTableSchema: (includeTableSchema: boolean) => void;

  exportKind: string;
  setExportKind: (_: string) => void;
}

export const useExportConfigStore = create<ExportConfigState>()(
  persist(
    (set) => ({
      // 默认初始状态
      delimiter: ",",
      includeHeader: true,
      jsonPretty: true,
      includeTableSchema: true,
      exportKind: "csv",

      setExportKind(exportKind) {
        set({ exportKind });
      },

      setIncludeTableSchema(includeTableSchema) {
        set({ includeTableSchema });
      },

      setJsonPretty(jsonPretty) {
        set({ jsonPretty });
      },

      // 实现 Actions
      setDelimiter: (delimiter) => set({ delimiter }),
      setIncludeHeader: (includeHeader) => set({ includeHeader }),
    }),
    {
      // 存储在 localStorage 中的 key
      name: "export-config-storage",
      // 默认即为 localStorage，createJSONStorage 确保数据以 JSON 格式正确存取
    },
  ),
);

export function useTabSize() {
  const { editor } = useUserSettings();
  return useMemo(() => {
    try {
      let x = JSON.parse(editor.editorOption);
      const value = x["tabSize"];

      if (value && typeof x === "number") {
        return value;
      }
    } catch (e) {
      console.error("parse editor.editorOption failed", e);
    }
    return 4;
  }, [editor]);
}

interface OneShotUI {
  ui: ReactElement;
  setUi: (ui: ReactElement, title?: string) => void;
  title?: string;
}

// used in place I don't hard to pass parameter mostly used in menu handle

export const useOneShotDialogUI = create<OneShotUI>()((set) => ({
  ui: null,
  diagProps: null,
  setUi: (ui, title: string) => set({ ui, title }),
  title: "",
}));

interface EditInModalLanguage {
  editingRowLanguage: string | null;
  setEditingRowLanguage: (lang: string | null) => void;
  clearEditingRowLanguage: () => void; // 记得在 interface 里加上这个方法
}

// 重点：不要在 create 后面加 <EditInModalLanguage>
export const useEditInModalLanguageStore = create<EditInModalLanguage>()(
  persist(
    (set) => ({
      editingRowLanguage: null,
      setEditingRowLanguage: (lang) => set({ editingRowLanguage: lang }),
      clearEditingRowLanguage: () => set({ editingRowLanguage: null }),
    }),
    {
      name: "edit-in-modal-language",
    },
  ),
);
export enum MongoDataViewModel {
  TableList = "table-list",
  Object = "object",
  Json = "json",
}
interface MongoViewModelState {
  viewModel: MongoDataViewModel;
  setViewModel: (
    mode: MongoDataViewModel | ((prev: MongoDataViewModel) => MongoDataViewModel),
  ) => void;
}

export const useMongoViewModelStore = create<MongoViewModelState>()(
  persist(
    (set) => ({
      // 初始状态，例如默认为 TableList
      viewModel: MongoDataViewModel.TableList,

      // 支持直接传值或函数式更新
      setViewModel: (input) =>
        set((state) => ({
          viewModel: typeof input === "function" ? input(state.viewModel) : input,
        })),
    }),
    {
      name: "mongo-view-model", // 存储在 localStorage 的 key
    },
  ),
);

export const useMongoEditorViewModelStore = create<MongoViewModelState>()(
  persist(
    (set) => ({
      // 初始状态，例如默认为 TableList
      viewModel: MongoDataViewModel.Object,

      // 支持直接传值或函数式更新
      setViewModel: (input) =>
        set((state) => ({
          viewModel: typeof input === "function" ? input(state.viewModel) : input,
        })),
    }),
    {
      name: "mongo-editor-model", // 存储在 localStorage 的 key
    },
  ),
);

interface SqlTableDataFilterState {
  filter: string;
  setFilter: (filter: string) => void;
}

export const useSqlTableFilterOp = create<SqlTableDataFilterState>()(
  persist(
    (set) => ({
      // 初始值设为 "="
      filter: "=",

      // 更新 filter 的方法
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // 存储在 localStorage 中的 key 名称
      name: "sql-table-filter-op",
      // (可选) 默认就是 localStorage，也可以指定为 sessionStorage
      // getStorage: () => localStorage,
    },
  ),
);

// 定义 MongoDB 表格过滤器的状态接口
interface MongoTableFilterState {
  filter: string; // 存储如 "$eq", "$ne", "$regex", "$gt" 等操作符
  setFilter: (filter: string) => void;
}

export const useMongoTableFilterOp = create<MongoTableFilterState>()(
  persist(
    (set) => ({
      // 默认操作符设为 "$eq" (等于)，这是 MongoDB 的标准等值查询
      filter: "$eq",

      // 更新操作符
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // 存储在 localStorage 中的唯一标识符
      name: "mongo-table-filter-op",
    },
  ),
);

interface SqlTableDataFilterState {
  filter: string;
  setFilter: (filter: string) => void;
}

export const useSqlDataFilter = create<SqlTableDataFilterState>()(
  persist(
    (set) => ({
      // 初始值设为 "="
      filter: "=",

      // 更新 filter 的方法
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // 存储在 localStorage 中的 key 名称
      name: "sql-data-filter-str",
      // (可选) 默认就是 localStorage，也可以指定为 sessionStorage
      // getStorage: () => localStorage,
    },
  ),
);
export enum CBQueryViewMode {
  Table = "table",
  Json = "json",
}

interface CBQueryViewState {
  viewMode: CBQueryViewMode;
  setViewMode: (mode: CBQueryViewMode) => void;
}

export const useCBQueryViewStore = create<CBQueryViewState>()(
  persist(
    (set) => ({
      // 預設值為 Json (對應你 load 裡面的邏輯)
      viewMode: CBQueryViewMode.Json,
      setViewMode: (viewMode) => set({ viewMode }),
    }),
    {
      name: "couchbase-query-view-mode",
    },
  ),
);
