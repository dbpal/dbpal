import { ReactNode, useCallback, useMemo } from "react";
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
import { CloseIcon } from "./icons";
import {
  AiOnlineSearch,
  AISteamResp,
  ChatHistory,
  commands,
  DBConnectionConfig,
  ExportKindXlsDatetimeformat,
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
  // Key format: "tableName:dbName" -> Visibility Model
  configs: Record<string, GridColumnVisibilityModel>;

  // Action to update visibility for a specific table instance
  setColumnVisibility: (
    tableName: string,
    dbName: string,
    model: GridColumnVisibilityModel,
  ) => void;

  // Helper to get visibility (returns empty object if not found)
  getColumnVisibility: (tableName: string, dbName: string) => GridColumnVisibilityModel;
}

export const useTableColumnVisibleStore = create<TableColumnVisible>()(
  persist(
    (set, get) => ({
      configs: {},

      setColumnVisibility: (tableName, dbName, model) => {
        const key = `${tableName}:${dbName}`;
        set((state) => ({
          configs: {
            ...state.configs,
            [key]: model,
          },
        }));
      },

      getColumnVisibility: (tableName, dbName) => {
        const key = `${tableName}:${dbName}`;
        return get().configs[key] || {};
      },
    }),
    {
      name: "mui-datagrid-columns-visible",
    },
  ),
);

export const useConfirmMsg = create<{
  confirmMsgs: Map<string, string>; // Specify the Map key and value types.
  addConfirmMsg: (id: string, msg: string) => void;
  removeConfirmMsg: (id: string) => void;
  getConfirmMsg: (id: string) => string | undefined;
}>((set, get) => ({
  // State: You can use Map directly; Zustand is not very restrictive.
  confirmMsgs: new Map(),

  getConfirmMsg: (id) => get().confirmMsgs.get(id),
  // Methods: update the values directly.
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
  matchPattern: string[];

  setMatchPattern: (matchPattern: string[]) => void;
}

// Move the <AppConfigState> generic here inside the persist wrapper
export const useMatcher = create<MatcherTreeState>()(
  persist(
    (set) => ({
      matchPattern: [],
      setMatchPattern: (xs) => set({ matchPattern: xs }),
    }),
    {
      name: "matcher-storage",
    },
  ),
);

interface BoolSwitchState {
  // Use Record instead of Map.
  switches: Record<string, boolean>;
  setSwitch: (key: string, value: boolean) => void;
  getSwitch: (key: string) => boolean;
}

export const useBoolSwitch = create<BoolSwitchState>()(
  persist(
    (set, get) => ({
      getSwitch: (key) => get().switches[key] || false,
      switches: {}, // The initial value is an empty object.
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
    // state is the latest state at the current moment.
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
      dbName: string,
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
        dbName,
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

export const useConnLoading = create<{
  loading: Set<string>;
  setLoading: (name: string, loading: boolean) => void;
  isLoading: (name: string) => boolean;
  removeLoading: (name: string) => void;
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
  removeLoading: (name) => {
    set((state) => {
      const nextSet = new Set(state.loading);
      nextSet.delete(name);
      return { loading: nextSet };
    });
  },
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
      name: "page-size-storage", // Key name used to store the value in localStorage.
      // storage: createJSONStorage(() => localStorage), // localStorage is the default and can be omitted.
    },
  ),
);

interface SizeState {
  // Core data, for example: { "sidebar": 300, "editor": 500 }.
  sizes: Record<string, number>;

  // Set the value for a specific key.
  setSize: (key: string, value: number) => void;

  // Get a value (supports a default to prevent undefined).
  getSize: (x: UseSizeConfig) => number;

  // Update multiple values or reset them.
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

      // Note: Zustand state is reactive, but getSize is a helper function.
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
  step: number;
  default: number;

  notice: ReactNode;

  static chatMemoryKey: UseSizeConfig = {
    key: "chat-memory-k",
    min: 0,
    default: 6,
    max: 20,
    step: 2,
    notice: "chat memory buffer size",
  };

  static gridCol: UseSizeConfig = {
    key: "grid-col",
    min: 1,
    max: 3,
    default: 1,
    step: 1,
    notice: "grid column count",
  };

  static editorHeight: UseSizeConfig = {
    key: "editor-heigh",
    min: 200,
    max: 1200,
    step: 10,
    default: 300,
    notice: "editor height",
  };

  static couchbaseQueryLimit: UseSizeConfig = {
    key: "couchbase-query-limit",
    min: 20,
    max: 10000,
    step: 10,
    default: 300,
    notice: "couchbase query limit",
  };

  static queryLimit: UseSizeConfig = {
    key: "query-limit",
    min: 1000,
    max: 10_00_00,
    step: 1000,
    default: 10000,
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
  // Key: "dbName-name" | Value: Unique array of strings
  filters: Record<string, string[]>;

  // Actions
  addFilter: (dbName: string, name: string, filterValue: string) => void;
  removeFilter: (dbName: string, name: string) => void;
  // Add the definition here
  getFilter: (dbName: string, name: string) => string[];
}

// Defining the creator separately helps TS resolve the 'Partial' conflict
const storeApi: StateCreator<TableFilterState, [["zustand/persist", unknown]]> = (
  set,
  get,
) => ({
  filters: {},

  addFilter: (dbName, name, filterValue) => {
    const key = `${dbName}-${name}`;
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

  getFilter: (dbName, name) => {
    const key = `${dbName}-${name}`;
    // get() now correctly recognizes the state structure
    return get().filters[key] || [];
  },

  removeFilter: (dbName, name) => {
    const key = `${dbName}-${name}`;
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

// 1. Define the data item interface (corresponding to SQL table columns).
interface ElementSize {
  width: number | null;
  height: number | null;
}

// 2. Define the Store state and action interfaces.
interface ElementSizeState {
  // Store values in an object, using name as the key and the data item as the value to simulate a primary-key index.
  elements: Record<string, ElementSize>;

  // Operations: create, delete, and update.
  setElement: (name, element: ElementSize) => void;
}

// 3. Create a Store with persistence.
export const useElementSizeStore = create<ElementSizeState>()(
  persist(
    (set) => ({
      elements: {}, // Initial state.

      // Implement set/update logic (simulating INSERT OR REPLACE).
      setElement: (name, element) =>
        set((state) => ({
          elements: {
            ...state.elements,
            [name]: element,
          },
        })),

      // Implement delete logic (simulating DELETE).
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

// Define the Store state and methods.
interface UserSettingsState extends UserSettings {
  // Actions
  updateAppearance: (data: Partial<UserSettings["appearance"]>) => void;

  updateEditor: (data: Partial<UserSettings["editor"]>) => void;
  updateMisc: (data: Partial<UserSettings["misc"]>) => void;

  updateProxy: (data: Partial<UserSettings["proxy"]>) => void;

  // Computed properties (handled by a selector or defined here as helper functions).
  getDatagridRowHeight: () => number;
}

const initialSettings: UserSettings = {
  appearance: {
    fontSize: 15,
    fontWeight: "bolder",
    fontFamily: "PT Mono",
    lightBgColor: "",
    darkBgColor: "",
    datagridRowHeightFactor: 3,
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

      // Update logic.
      updateAppearance: (data) =>
        set((state) => ({ appearance: { ...state.appearance, ...data } })),

      updateEditor: (data) =>
        set((state) => ({ editor: { ...state.editor, ...data } })),

      updateMisc: (data) => set((state) => ({ misc: { ...state.misc, ...data } })),

      // Replaces the original datagridRowHeight() method.
      getDatagridRowHeight: () => {
        const { fontSize, datagridRowHeightFactor } = get().appearance;
        return Math.ceil(fontSize * datagridRowHeightFactor);
      },
    }),
    {
      name: userSettingKey, // Consider combining these into one key for easier management.
      version: 1,
    },
  ),
);

export const useFontSize = () => {
  return useUserSettings((s) => s.appearance.fontSize);
};

export const useDatagridHeight = () => {
  return useUserSettings((state) =>
    Math.ceil(state.appearance.fontSize * state.appearance.datagridRowHeightFactor),
  );
};

export const useMatcherFn = () => {
  // 1. Fetch only the data (this re-renders only when the data changes).
  const matchPattern = useMatcher((s) => s.matchPattern);

  // 2. Define the logic function externally.
  const matcher = useCallback(
    (name: string) => {
      if (matchPattern.length === 0) {
        return true;
      }
      for (const n of matchPattern) {
        // Optimization: wcmatch can be moved outside the loop or memoized; keep it as-is here for consistency.
        const matchFn = wcmatch("*" + n + "*", { flags: "i" });
        if (matchFn(name)) {
          return true;
        }
      }
      return false;
    },
    [matchPattern],
  ); // The matcher function reference changes only when the pattern changes.

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
        console.log("Start syncing data...");
        return (state, error) => {
          if (error) {
            console.error("Sync failed:", error);
          } else {
            console.log("Sync successful, data count:", state?.connections?.length);
          }
        };
      },
    },
  ),
);

export interface QueryLog {
  id?: number;
  dbName: string;
  name: string; // connection name
  sql: string;
  create_at: number; // Store the timestamp.
  success: number; // 1 or 0.
}

interface QueryLogState {
  // Core operation methods.
  save: (dbName: string, sql: string, success: boolean) => Promise<void>;
  query: (dbName: string) => Promise<QueryLog[]>;
  deleteByUuid: (dbName: string) => Promise<number>;
  deleteByName: (name: string) => Promise<void>;
}

const maxQueryLogCount = 10000;

export const useQueryLogStore = create<QueryLogState>(() => ({
  // Save the log.
  save: async (dbName, sql, success) => {
    try {
      // 1. Save the new log
      await db.query_log.add({
        dbName,
        name: extractConfigNameFromUuid(dbName),
        sql,
        success: success ? 1 : 0,
        create_at: Date.now(),
      });

      // 2. Check the count for this specific dbName
      const count = await db.query_log.where("dbName").equals(dbName).count();

      // 3. If over limit, delete the oldest ones
      const LIMIT = maxQueryLogCount;
      if (count > LIMIT) {
        const extraCount = count - LIMIT;

        // Find the IDs of the oldest records for this dbName
        const oldestLogs = await db.query_log
          .where("dbName")
          .equals(dbName)
          .limit(extraCount) // Pick the first 'n' records (oldest)
          .primaryKeys(); // Get just the IDs to speed up deletion

        await db.query_log.bulkDelete(oldestLogs);
        console.info(`Trimmed ${extraCount} old logs for dbName: ${dbName}`);
      }
    } catch (e) {
      console.error("save query log failed:", e);
    }
  },

  query: async (dbName) => {
    let collection = db.query_log
      .where("dbName")
      .equals(dbName)
      .reverse()
      .sortBy("create_at");
    return collection;
  },

  // Delete.
  deleteByUuid: async (dbName) => {
    return await db.query_log.where("dbName").equals(dbName).delete();
  },

  deleteByName: async (name) => {
    await db.query_log.where("name").equals(name).delete();
  },
}));

export interface FavoriteSql {
  id?: number;
  dbName: string;
  name: string;
  kind: string;
  sql: string;
  connection_name: string;
  last_modified: number;
}

export class MyDatabase extends Dexie {
  query_log!: Table<QueryLog>;
  favorite_sql!: Table<FavoriteSql>;
  chat_history!: Table<ChatMessage>; // Add the chat table.

  constructor() {
    super("AppDatabase");
    this.version(2).stores({
      // Remember to increment the version number.
      query_log: "++id,id,dbName,name,[dbName+create_at]",
      favorite_sql: "++id, dbName, name, connection_name, &[dbName+name]",
      // Added: Index name and timestamp for easier sorting and cleanup.
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
  getByUuid: (dbName?: string) => Promise<FavoriteSql[]>;
  getDistinctUuids: () => Promise<string[]>;
  deleteById: (id: number) => Promise<void>;
  clearByUuid: (dbName: string) => Promise<void>;
  deleteByConnectionName: (name: string) => Promise<void>;
}

export const useFavoriteStore = create<FavoriteStore>(() => ({
  // REPLACE INTO logic using db.put()

  save: async (data) => {
    const LIMIT_PER_UUID = 500; // Favorites usually have a smaller limit than logs
    const connection_name = extractConfigNameFromUuid(data.dbName);

    try {
      // Use a Read/Write transaction for atomicity
      await db.transaction("rw", db.favorite_sql, async () => {
        // 1. Check for existing record (to simulate REPLACE)
        const existing = await db.favorite_sql
          .where("[dbName+name]")
          .equals([data.dbName, data.name])
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
          const count = await db.favorite_sql
            .where("dbName")
            .equals(data.dbName)
            .count();

          if (count > LIMIT_PER_UUID) {
            const extraCount = count - LIMIT_PER_UUID;

            // Find the oldest favorites for this dbName
            // We use 'last_modified' or 'id' to determine "oldest"
            const toDelete = await db.favorite_sql
              .where("dbName")
              .equals(data.dbName)
              .limit(extraCount)
              .primaryKeys();

            await db.favorite_sql.bulkDelete(toDelete);
            console.info(
              `Limit reached. Trimmed ${extraCount} favorites for ${data.dbName}`,
            );
          }
        }
      });
    } catch (e) {
      console.error("Save favorite failed:", e);
      throw e;
    }
  },

  getByUuid: async (dbName) => {
    if (!dbName) return await db.favorite_sql.toArray();
    return await db.favorite_sql.where("dbName").equals(dbName).toArray();
  },
  getDistinctUuids: async () => {
    // Check the total count first.
    const count = await db.favorite_sql.count();
    if (count === 0) return [];

    // Sort by the index only when there is data.
    const uuids = await db.favorite_sql.orderBy("dbName").uniqueKeys();
    return uuids as string[];
  },

  deleteById: async (id) => {
    await db.favorite_sql.delete(id);
  },

  clearByUuid: async (dbName) => {
    await db.favorite_sql.where("dbName").equals(dbName).delete();
  },

  deleteByConnectionName: async (name) => {
    await db.favorite_sql.where("connection_name").equals(name).delete();
  },
}));

export interface ChatMessage {
  id?: number;
  name: string;
  role: "user" | "assistant" | "system"; // Added: Define the role type.
  content: string;
  timestamp: number;
}

interface ChatState {
  maxLogs: number;
  messagesByNames: Record<string, ChatMessage[]>;
  isLoading: boolean;

  fetchHistory: (name: string) => Promise<ChatMessage[]>;
  // Updated: Add the role parameter.
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
          role, // Save the role information.
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
      name: "view-in-modal-storage-match-str", // Unique key used to store the value in localStorage.
    },
  ),
);

interface ExportConfigState {
  delimiter: string; // Note: Keep the spelling "delimiter" as defined.
  includeHeader: boolean;

  setDelimiter: (delimiter: string) => void;
  setIncludeHeader: (includeHeader: boolean) => void;

  jsonPretty: boolean;
  setJsonPretty: (jsonPretty: boolean) => void;

  includeTableSchema: boolean;

  setIncludeTableSchema: (includeTableSchema: boolean) => void;

  exportKind: string;
  setExportKind: (_: string) => void;

  xlsxDateTimeFormat: ExportKindXlsDatetimeformat;

  setXlsxDatetimeFormat: (_: ExportKindXlsDatetimeformat) => void;
}

export const useExportConfigStore = create<ExportConfigState>()(
  persist(
    (set) => ({
      // Default initial state.
      delimiter: ",",
      includeHeader: true,
      jsonPretty: true,
      includeTableSchema: true,
      exportKind: "csv",
      xlsxDateTimeFormat: {
        datatime: "yyyy-mm-dd hh:mm:ss",
        date: "yyyy-mm-dd",
        time: "hh:mm:ss",
      },
      setXlsxDatetimeFormat: (x) => {
        set({ xlsxDateTimeFormat: x });
      },

      setExportKind(exportKind) {
        set({ exportKind });
      },

      setIncludeTableSchema(includeTableSchema) {
        set({ includeTableSchema });
      },

      setJsonPretty(jsonPretty) {
        set({ jsonPretty });
      },

      // Implement the actions.
      setDelimiter: (delimiter) => set({ delimiter }),
      setIncludeHeader: (includeHeader) => set({ includeHeader }),
    }),
    {
      // Key used to store the value in localStorage.
      name: "export-config-storage",
      // localStorage is the default; createJSONStorage ensures data is read and written as JSON.
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

interface EditInModalLanguage {
  editingRowLanguage: string | null;
  setEditingRowLanguage: (lang: string | null) => void;
  clearEditingRowLanguage: () => void; // Remember to add this method to the interface.
}

// Important: Do not add <EditInModalLanguage> after create.
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
      // Initial state, for example, TableList by default.
      viewModel: MongoDataViewModel.TableList,

      // Supports a direct value or a functional update.
      setViewModel: (input) =>
        set((state) => ({
          viewModel: typeof input === "function" ? input(state.viewModel) : input,
        })),
    }),
    {
      name: "mongo-view-model", // Key used to store the value in localStorage.
    },
  ),
);

export const useMongoEditorViewModelStore = create<MongoViewModelState>()(
  persist(
    (set) => ({
      // Initial state, for example, TableList by default.
      viewModel: MongoDataViewModel.Object,

      // Supports a direct value or a functional update.
      setViewModel: (input) =>
        set((state) => ({
          viewModel: typeof input === "function" ? input(state.viewModel) : input,
        })),
    }),
    {
      name: "mongo-editor-model", // Key used to store the value in localStorage.
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
      // Set the initial value to "=".
      filter: "=",

      // Method for updating the filter.
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // Key name used to store the value in localStorage.
      name: "sql-table-filter-op",
      // (Optional) Defaults to localStorage; sessionStorage can also be specified.
      // getStorage: () => localStorage,
    },
  ),
);

// Define the state interface for the MongoDB table filter.
interface MongoTableFilterState {
  filter: string; // Store operators such as "$eq", "$ne", "$regex", and "$gt".
  setFilter: (filter: string) => void;
}

export const useMongoTableFilterOp = create<MongoTableFilterState>()(
  persist(
    (set) => ({
      // Set the default operator to "$eq" (equal), MongoDB's standard equality query.
      filter: "$eq",

      // Update the operator.
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // Unique identifier used to store the value in localStorage.
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
      // Set the initial value to "=".
      filter: "=",

      // Method for updating the filter.
      setFilter: (filter: string) => {
        set({ filter });
      },
    }),
    {
      // Key name used to store the value in localStorage.
      name: "sql-data-filter-str",
      // (Optional) Defaults to localStorage; sessionStorage can also be specified.
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
      // Default to Json (matching the logic in load).
      viewMode: CBQueryViewMode.Json,
      setViewMode: (viewMode) => set({ viewMode }),
    }),
    {
      name: "couchbase-query-view-mode",
    },
  ),
);

// State type.
interface RedisKeyFilter {
  filters: Record<string, string>;
  // Common operations.
  setFilter: (key: string, value: string) => void;
  removeFilter: (key: string) => void;
}

export const useRedisKeyFilter = create<RedisKeyFilter>()(
  persist(
    (set, get) => ({
      filters: {},
      removeFilter: (key) => {
        const next = { ...get().filters };
        delete next[key];
        set({ filters: next });
      },
      setFilter: (key, value) =>
        set((state) => ({
          filters: { ...state.filters, [key]: value },
        })),
    }),
    {
      name: "redis-key-filter",
    },
  ),
);

interface RedisMaxKeyReturns {
  maxRow: number;
  setMaxRow: (x: number) => void;
}
export const useRedisMaxKeyStore = create<RedisMaxKeyReturns>()(
  persist(
    (set) => ({
      maxRow: 50000, // Default value; change as needed.
      setMaxRow: (x) => set({ maxRow: x }),
    }),
    {
      name: "redis-max-key", // localStorage key.
    },
  ),
);
