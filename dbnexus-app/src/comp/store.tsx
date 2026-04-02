import { ReactElement, ReactNode, useCallback, useMemo } from "react";
import { create, StateCreator } from "zustand";
import {
  appStore,
  DataSourceKind,
  extractConfigNameFromUuid,
  generateRandomString,
  LLMConfig,
  TabContent,
} from "./helper";
import { AlertColor, Typography } from "@mui/material";
import React from "react";
import { persist, PersistStorage, StorageValue } from "zustand/middleware";
import Dexie, { type Table } from "dexie";
import wcmatch from "wildcard-match";

class TauriStoreManger<S> implements PersistStorage<S> {
  constructor(
    public setBefore?: (_: S) => Promise<S>,
    public afterFetch?: (_: S) => Promise<void>,
  ) {}
  async setItem(name: string, value: StorageValue<S>) {
    if (this.setBefore) {
      value.state = await this.setBefore(value.state);
    }
    return await appStore.set(name, value);
  }

  async getItem(name: string) {
    let ret = await appStore.get<StorageValue<S>>(name);
    if (this.afterFetch) {
      await this.afterFetch(ret.state);
    }
    return ret;
  }

  removeItem(name: string) {
    appStore.delete(name);
  }
}

import { GridColumnVisibilityModel } from "@mui/x-data-grid";
import { PassWordManager } from "./Common";
import types from "./ApiStruct";
import { editor } from "monaco-editor";
import { Channel, invoke } from "@tauri-apps/api/core";
import { cancelTask } from "./handlers";

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
      name: "mui-datagrid-configs",
    },
  ),
);

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
      },
      setConfig: (config) => set({ config }),
    }),

    {
      name: "llm-config",
      version: 1,
      storage: new TauriStoreManger<LLMStore>(
        async (s) => {
          if (s.config.apiKey) {
            await PassWordManager.savePassword(LLMConfig.key, s.config.apiKey, true);
          } else {
            await PassWordManager.deletePassword(LLMConfig.key);
          }
          return { ...s, apiKey: "" };
        },
        async (s) => {
          let pass = await PassWordManager.getPassword(LLMConfig.key);
          s.config.apiKey = pass;
          Object.setPrototypeOf(s.config, LLMConfig.prototype);
        },
      ),
    },
  ),
);

export const makeLLMCallable = (config: LLMConfig) => {
  const makeReq = (msg: string, images): types.OpenAIReq => {
    return {
      url: config.baseUrl,
      api_key: config.apiKey,
      model: config.model,
      msg,
      temperature: config.temperature,
      max_tokens: config.maxToken,
      top_p: config.topP,
      images,
    };
  };

  return {
    invoke: async (msg: string): Promise<string> => {
      return await invoke<string>("openai_invoke", {
        req: makeReq(msg, []),
      });
    },
    stream: async (
      uuid: string,
      queryId: string,
      msg: string,
      images: string[],
      handleMessage: (event: string) => void,
      handleCancel: (cancel: () => void) => void,
    ) => {
      const taskId = generateRandomString();
      const emit = new Channel<string>((event: string) => {
        handleMessage(event);
      });
      handleCancel(() => {
        cancelTask(taskId);
      });
      await invoke("openai_stream", {
        req: makeReq(msg, images),
        emit,
        taskId,
        name: uuid,
        queryId,
      });
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
      name: "couchbase-ui-sizes",
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
    min: 100,
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

interface TableFilterState {
  filters: Record<string, string[]>;
  addFilter: (uuid: string, name: string, filterValue: string) => void;
  removeFilter: (uuid: string, name: string) => void;
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

// 1. Define your State interface
interface AIPromptState {
  prompts: Record<DataSourceKind, string>;
  getPrompt: (kind: DataSourceKind) => string | null;
  setPrompt: (kind: DataSourceKind, prompt: string) => void;
  removePrompt: (kind: DataSourceKind) => void;
}

// 2. Use the "Curried" version of create for better TS support with middleware
export const useAIPrompt = create<AIPromptState>()(
  persist(
    (set, get) => ({
      prompts: {} as Record<DataSourceKind, string>, // Initialize with cast if needed

      removePrompt: (kind) =>
        set((state) => ({
          prompts: {
            ...state.prompts,
            [kind]: "",
          },
        })),
      getPrompt: (kind) => {
        return get().prompts[kind] ?? null;
      },
      setPrompt: (kind, prompt) =>
        set((state) => ({
          prompts: {
            ...state.prompts,
            [kind]: prompt,
          },
        })),
    }),
    {
      name: "aui-prompt-storage",
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
    intputVariant: string;
    theme: string;
    zoom: number;
  };
  export_: {
    csvDelimiter: string;
    includeHeader: boolean;
  };
  editor: {
    tabSize: number;
    theme: string;
    editorOption: string;
  };
  misc: {
    autoStart: boolean;
    logLevel: string;
  };
}

// 定义 Store 的状态和方法
interface UserSettingsState extends UserSettings {
  // Actions
  updateAppearance: (data: Partial<UserSettings["appearance"]>) => void;
  updateExport: (data: Partial<UserSettings["export_"]>) => void;
  updateEditor: (data: Partial<UserSettings["editor"]>) => void;
  updateMisc: (data: Partial<UserSettings["misc"]>) => void;

  // 计算属性 (由 Selector 处理，或在此定义辅助函数)
  getDatagridRowHeight: () => number;
}

const initialSettings: UserSettings = {
  appearance: {
    fontSize: 15,
    fontWeight: "normal",
    fontFamily: "",
    lightBgColor: "",
    darkBgColor: "",
    datagridRowHeightFactor: 2.5,
    intputVariant: "standard",
    theme: "light",
    zoom: 1,
  },
  export_: {
    csvDelimiter: ",",
    includeHeader: true,
  },
  editor: {
    tabSize: 4,
    theme: "",
    editorOption: "{\n}",
  },
  misc: {
    autoStart: false,
    logLevel: "info",
  },
};

export const useUserSettings = create<UserSettingsState>()(
  persist(
    (set, get) => ({
      ...initialSettings,

      // 更新逻辑
      updateAppearance: (data) =>
        set((state) => ({ appearance: { ...state.appearance, ...data } })),

      updateExport: (data) =>
        set((state) => ({ export_: { ...state.export_, ...data } })),

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
      name: "dbpal-combined-settings", // 建议合并成一个 key，方便管理
      storage: new TauriStoreManger(), // 使用你之前的 Tauri 存储实现
      // 如果需要处理旧版本的 key 迁移，可以在这里写 migrate
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
  connections: types.DBConnectionConfig[];

  addConnection: (config: types.DBConnectionConfig) => Promise<void>;
  removeConnection: (name: string) => void;
  nameExists: (name: string) => boolean;
  getConnection: (name: string) => Promise<types.DBConnectionConfig | null>;
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
          password = await PassWordManager.getPassword(name);
          if (false === conn[savePasswordKey] && !password) {
            password = await window.prompt("please input password:");
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
        PassWordManager.deletePassword(name);
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
    },
  ),
);

interface EditorState {
  values: Record<string, editor.IStandaloneCodeEditor>;
  getEditor: (arg0: string) => editor.IStandaloneCodeEditor | null;
  setEditor: (arg1: editor.IStandaloneCodeEditor) => void;
  removeEditor: (arg0: string) => void;
}

export const useEditorState = create<EditorState>((set, get) => ({
  values: {},
  getEditor: (uuid) => {
    return get().values[uuid];
  },
  setEditor: (e) => {
    const id = e.getModel().id;
    let _new = { ...get() };
    _new.values[id] = e;
    set(_new);
  },

  removeEditor: (uuid) => {
    const values = get().values;
    delete values[uuid];
    set({ values });
  },
}));

export interface QueryLog {
  id?: number;
  uuid: string;
  name: string; // connecti0on name
  sql: string;
  language?: string;
  create_at: number; // 存储时间戳
  success: number; // 1 或 0
}

interface QueryLogState {
  // 核心操作方法
  save: (
    uuid: string,
    sql: string,
    success: boolean,
    language?: string,
  ) => Promise<void>;
  query: (uuid: string) => Promise<QueryLog[]>;
  deleteByUuid: (uuid: string) => Promise<number>;
  deleteByName: (name: string) => Promise<void>;
}

const maxQueryLogCount = 10000;

export const useQueryLogStore = create<QueryLogState>(() => ({
  // 保存日志
  save: async (uuid, sql, success, language) => {
    try {
      // 1. Save the new log
      await db.query_log.add({
        uuid,
        name: extractConfigNameFromUuid(uuid),
        sql,
        language,
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

  // 分页查询 (模拟 SQL: like, order by, limit, offset)
  query: async (uuid) => {
    let collection = db.query_log
      .where("uuid")
      .equals(uuid)
      .reverse()
      .sortBy("create_at ");
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
  language?: string;
  last_modified: number;
}

export class MyDatabase extends Dexie {
  query_log!: Table<QueryLog>; // Previous table
  favorite_sql!: Table<FavoriteSql>;

  constructor() {
    super("AppDatabase");
    this.version(1).stores({
      query_log: "++id,id,uuid,name,[uuid+create_at]",
      // [uuid+name] creates the unique constraint logic
      favorite_sql: "++id, uuid, name, connection_name, &[uuid+name]",
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
    // Dexie's uniqueKeys() is very fast for DISTINCT operations
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
