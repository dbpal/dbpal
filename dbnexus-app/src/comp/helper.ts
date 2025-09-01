import { editor, languages, Position } from "monaco-editor";
import types from "./ApiStruct";
import { createContext, CSSProperties, ReactElement, ReactNode } from "react";
import { ask } from '@tauri-apps/plugin-dialog';
import { load } from "@tauri-apps/plugin-store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import { BSON, EJSON } from "bson";


interface ConfirmOptions {
  title: string;
  content?: string;
  confirmText?: string;
  cancelText?: string;
}

async function secondConfirm({
  title,
  content,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
}: ConfirmOptions): Promise<boolean> {
  return ask(title, {
    title: content,
    kind: 'error',
    okLabel: confirmText,
    cancelLabel: cancelText,
  });
}


function generateRandomString(length: number = 10): string {
  const characters =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  for (let i = 0; i < length; i++) {
    const randomIndex = Math.floor(Math.random() * characters.length);
    result += characters[randomIndex];
  }
  return result;
}

// redis的数据存储的时候都是二进制 有时候包含的是有效的
// utf8字符前台的处理的最好当成utf8来处理
// 不然当成二进制数据来处理呢
// 后台放回数据的时候都是 hex string模式

function hexTryToString(hex): string | Uint8Array {
  if (!hex) {
    return '';
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  // 将字节数组转换为 UTF-8 字符串
  let utf8String: string;

  try {
    utf8String = new TextDecoder('utf-8', { fatal: true }).decode(
      new Uint8Array(bytes)
    );
  } catch {
    return new Uint8Array(bytes);
  }
  return utf8String;
}
function uint8ArrayToUtf8(uint8Array): string {

  const decoder = new TextDecoder('utf-8');
  const str = decoder.decode(uint8Array);

  return str


}

function hexToString(hex: string): string {
  if (!hex) {
    return '';
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  return new TextDecoder('utf-8').decode(new Uint8Array(bytes));
}


function stringToHexArray(str: string): Uint8Array {
  // 创建一个 Uint8Array 来存储字节
  const bytes = new TextEncoder().encode(str);
  return bytes;
}

function arrayToHex(uint8Array: Uint8Array | number[]) {
  return Array.from(uint8Array) // 将 Uint8Array 转换为普通数组
    .map((byte) => byte.toString(16).padStart(2, '0')) // 转换每个字节为十六进制并补齐
    .join(''); // 连接为一个字符串
}

function stringToHex(str): string {
  return arrayToHex(stringToHexArray(str));
}


function backendCompletionItemToEditor(
  xs: types.CompletionItem[]
): languages.CompletionList {
  function getKind(kind: types.CompletionKind): languages.CompletionItemKind {
    switch (kind) {
      case 'Command':
        return languages.CompletionItemKind.Keyword;
      case 'KeyWord':
        return languages.CompletionItemKind.Keyword;
      case 'Function':
        return languages.CompletionItemKind.Function;
      case 'Database':
        return languages.CompletionItemKind.Module;
      case 'Schema':
        return languages.CompletionItemKind.Folder;
      case 'Table':
      case "View":
        return languages.CompletionItemKind.Reference;
      case 'Column':
        return languages.CompletionItemKind.Field;
      case 'Text':
        return languages.CompletionItemKind.Text;
      case 'Type':
        return languages.CompletionItemKind.Class;
      case 'Index':
        return languages.CompletionItemKind.Reference;
      case 'Sequence':
        return languages.CompletionItemKind.Reference;
      case 'Stage':
        return languages.CompletionItemKind.Reference;
      case 'Trigger':
        return languages.CompletionItemKind.Function;
      case 'Event':
        return languages.CompletionItemKind.Event;
      case 'Procedure':
        return languages.CompletionItemKind.Function;
      case 'Extension':
        return languages.CompletionItemKind.Module;
      case 'User':
        return languages.CompletionItemKind.User;
      case 'Role':
        return languages.CompletionItemKind.User;
      case 'Constraint':
        return languages.CompletionItemKind.Operator;
      case 'Partition':
        return languages.CompletionItemKind.Constant;
      case 'PrepareStatement':
        return languages.CompletionItemKind.Function;
      case 'Pragma':
        return languages.CompletionItemKind.Keyword;
      case 'Policy':
        return languages.CompletionItemKind.Keyword;
      case 'Variable':
        return languages.CompletionItemKind.Variable;
      case 'Rule':
        return languages.CompletionItemKind.Operator;
      case 'ClickHouseTableProjection':
        return languages.CompletionItemKind.Property;
      case 'Prepare':
        return languages.CompletionItemKind.Reference;
      case 'Module':
        return languages.CompletionItemKind.Module;

    }
  }
  return {
    suggestions: xs.map((e): languages.CompletionItem => {
      return {
        label: e.item,
        kind: getKind(e.kind),
        insertText: e.item,
        range: undefined,
        detail: e.desc,
        documentation: e.desc
      };
    }),
  };
}

function extractConfigNameFromUuid(uuid: string): string {
  if (uuid.includes('/')) {
    return uuid.substring(0, uuid.indexOf('/'));
  } else {
    return uuid;
  }
}
function getViewportSize() {
  const width = window.innerWidth; // 获取视口的宽度
  const height = window.innerHeight; // 获取视口的高度

  return { width, height }; // 返回一个对象，包含宽度和高度
}

function calMenuStyle(e, eheigth: number): CSSProperties {
  e.preventDefault();
  const x = e.clientX;
  const y = e.clientY;
  let ret: CSSProperties = {};
  const leftSizeMenuDom = document.getElementById('menu-div');
  leftSizeMenuDom.style.display = 'block';
  const { height, width } = getViewportSize();
  ret.top = 'auto';
  ret.bottom = 'auto';
  ret.left = 'auto';
  ret.right = 'auto';
  if (width - x > 250) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      'px';
  } else {
    ret.right = width - x - 10 + 'px';
  }

  if (height - y > (eheigth)) {
    // 这个是点击的位置下面有很大的空间
    ret.top = y - 10 + 'px';
  } else {
    ret.bottom = height - y - 10 + 'px';
  }
  return ret;
}

function chunkArray(array: any[], chunkSize: number): any[][] {
  const result = [];
  for (let i = 0; i < array.length; i += chunkSize) {
    result.push(array.slice(i, i + chunkSize));
  }
  return result;
}

function errToString(message) {
  if (typeof message == 'object' && 'reason' in message) {
    // TODO show message
    message = message['reason'];
  }
  if (typeof message == 'object' && message instanceof Error) {
    // TODO show message
    message = message.toString();
  }
  return message;
}

enum DataSourceKind {
  Mysql = "mysql",
  MariaDB = "mariadb",
  TiDB = "tidb",
  Sqlite = "sqlite",
  Postgres = "postgres",
  MsSql = "mssql",
  Redis = "redis",
  Mongo = "mongo",
  Oracle = "oracle",
  Cassandra = "cassandra",
  ClickHouse = "clickhouse",
  LibSql = "libsql",
  FireBird = "firebird",
  DuckDB = "duckdb",
  Cockroach = "cockroach"
}

function quoteStyle(x: DataSourceKind): string {
  switch (x) {
    case DataSourceKind.MariaDB:
    case DataSourceKind.TiDB:
    case DataSourceKind.Mysql:
    case DataSourceKind.ClickHouse:
      return '`'
    default:
      return '"'
  }
}


function connTypeToDataSourceKind(name: string): DataSourceKind {
  return name as DataSourceKind
}
function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  return toUpper.includes('TEXT') || toUpper.includes("LONG") || toUpper.includes("CLOB")
}
const appStore = await load('store.json');

enum AppStoreKey {
  UserSetttings = 'user_settings',
  ConnectionConfig = 'connection_config',
  ColumnWidths = 'column_widths',
  FavoriteSql = "favorite_sql",
  Sizes = "sizes",
  Zoom = "zoom"
}


class SaveSizesToStore {
  static async saveSize(key: string, size: UISize) {
    let sizes = await appStore.get<object>(AppStoreKey.Sizes);
    if (!sizes) {
      sizes = {};
    }
    sizes[key] = size;
    await appStore.set(AppStoreKey.Sizes, sizes);
  }
  static async getSize(key: string): Promise<undefined | UISize> {
    return appStore.get(AppStoreKey.Sizes).then(s => s && s[key])
  }
}


class FavoriteSql {
  constructor(public uuid: string, public name: string, public kind: DataSourceKind, public sql: string, public lastModified: Date = new Date()) { }
  static key = AppStoreKey.FavoriteSql;
  static async save(x: FavoriteSql, showNotification) {
    try {

      let logs = await appStore.get<object>(FavoriteSql.key);
      if (!logs) {
        logs = {};
      }
      logs[FavoriteSql.formatKey(x.uuid, x.name)] = x;
      await appStore.set(FavoriteSql.key, logs)
    } catch (e) {
      showNotification('error', "save favorite sql failed", e);
      return
    }
    showNotification('success', "save sql Ok")
  }
  static formatKey(uuid: string, name: string) {
    return uuid + '-' + name
  }

  static async delete(key: string, showNotification) {
    try {
      let logs = await appStore.get<object>(FavoriteSql.key);
      if (!logs) {
        logs = {};
      }
      delete logs[key];
      await appStore.set(FavoriteSql.key, logs)
    } catch (e) {
      showNotification('error', "save favorite sql failed", e);
      return
    }
    showNotification('success', "delete sql Ok")
  }

  static async deleteByUuid(uuid: string) {
    try {

      let logs = await appStore.get<object>(FavoriteSql.key);
      if (logs) {
        logs = {};
      }

      for (var key in logs) {
        const log = logs[key] as FavoriteSql;
        if (log.uuid == uuid) {
          delete logs[key]
        }
      }
      await appStore.set(FavoriteSql.key, logs)
    } catch (e) {
      console.log("delete all logs belong to a key failed", e);
      return
    }
  }
  static async clear(showNotification) {
    try {
      await appStore.set(FavoriteSql.key, {});
    } catch (e) {
      showNotification('error', "save favorite sql failed", e);
      return
    }
    showNotification('success', "clean OK");
  }
}

class ColumnWidths {
  widths = {};
  constructor() {
    appStore.get(AppStoreKey.ColumnWidths).then(e => {
      if (e) {
        this.widths = e;
      }
    })

  }

  saveColumnWidths(name: string, width: number) {
    this.widths[name] = width;
    this.save();

  }
  save() {
    appStore.set(AppStoreKey.ColumnWidths, this.widths).catch(e => {
      console.log('save column width failed', e);
    });
  }
  getWidth(key) {
    return this.widths[key]
  }
  reset() {
    this.widths = {};
    this.save();

  }
}
var columnWidths = new ColumnWidths();
const VisiableContext = createContext(true);
const handleCopyToClipboard = (text, showNotification) => {
  writeText(text)
    .then(() => {
      showNotification('success', 'success copy to clipboard.');
    })
    .catch((e) => {
      showNotification('error', 'copy to clipboard failed', e);
    });
};


class TabContent {
  constructor(
    public id: string,
    public header: ReactNode,
    public content: ReactNode,
    public icon: ReactElement,
    public closeConfirmText?: string,
  ) { }
}
const TabsSetContext = createContext<{
  tabs: TabContent[];
  setTabs;
  setSelectTab;
}>(null);

const SetCloseConfirmTextContext = createContext<(x: string) => void>(null);

const ConnTypeContext = createContext<{
  kind: DataSourceKind;
}>(null);


const MatchPatternContext = createContext<(x: string) => boolean>(() => {
  return true
});


const UuidContext = createContext<{
  uuid: string;
}>(null);

const ContextMenuContext = createContext<{
  setUpMenu: ({ content, style }) => void;
}>(null);


interface ErrAndLoc {
  line?: number,
  column?: number,
  msg: string,
}

function validJson(s: string): Promise<ErrAndLoc | null> {
  return invoke<ErrAndLoc | null>("valid_json", { jsonStr: s })
}

function resetMuiDataRespWidth(old: types.MuiDataGridData): types.MuiDataGridData {
  return {
    sql: old.sql,
    data: old.data,
    fields: old.fields.map((e) => {
      return { width: undefined, ...e } as types.Field;
    }),
    total: old.total,
    cache_time: old.cache_time,
    has_more: old.has_more,
  };
}

type UISize = {
  width?: number;
  height?: number;
};
const gridStyle = { height: 'auto', maxHeight: '80vh' } as CSSProperties;

function saveAndSetSize(dom: Element, key: string, set: "height" | "width" | "both" = "both") {
  let e = new ResizeObserver((en) => {
    const height = en[0].target.clientHeight;
    const width = en[0].target.clientWidth;
    SaveSizesToStore.saveSize(key, { width, height });
  });
  SaveSizesToStore.getSize(key).then(size => {
    if (size) {
      let div = (dom as HTMLDivElement);
      if (set == "height" || set == "both") {
        div.style.height = size.height + 'px';
      }
      if (set == "width" || set == "both") {
        div.style.width = size.width + 'px';
      }
    }
  });
  e.observe(dom);
  return () => {
    e.unobserve(dom);
  };
}


class DocsAndCount {
  constructor(public docs: BSON.Document[], public count: number) { }
}


class MongoTableHandler {
  static reloadTableList(uuid: string, table) {
    return async (filter, sort, page, pageSize): Promise<types.MuiDataGridData> => {
      return await invoke('mongo_show_collection_table_list', {
        name: uuid,
        tableName: table,
        filter: BSON.serialize(filter),
        sort: BSON.serialize(sort),
        page,
        pageSize,
      });
    }
  }
  static reloadDocs(uuid: string, table) {
    return async (filter, sort, page, pageSize): Promise<DocsAndCount> => {
      try {
        let x = await invoke('mongo_show_collection_docs', {
          name: uuid,
          tableName: table,
          filter: BSON.serialize(filter),
          sort: BSON.serialize(sort),
          page,
          pageSize,
        });
        //@ts-ignore
        let docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
        //@ts-ignore
        let count = x.count;
        return new DocsAndCount(docs, count);
      } catch (e) {
        throw e;
      }
    }
  }

  static loadOne(uuid: string, table) {
    return async (id): Promise<BSON.Document> => {
      try {
        let x = await invoke<number[]>('mongo_get_one_document_by_id', {
          name: uuid,
          tableName: table,
          id: JSON.stringify(EJSON.serialize(id)),
        });
        return BSON.deserialize(new Uint8Array(x));
      } catch (e) {
        throw e;
      }
    }
  }
}


class UserSettings {
  theme: string = 'light';
  pageSize: number = 100;
  fontSize: number = 15;
  fontWeight: string = 'normal';
  fontFamily: string = undefined;
  tabSize: number = 4;
  editorOption: editor.IStandaloneEditorConstructionOptions = {};
  editorTheme(): string {
    return this.theme === 'dark' ? 'vs-dark' : 'light';
  }
  // 可能和以前的设置冲突
  datagridRowHeightXXX() {
    return Math.ceil(this.fontSize * 1.9)
  }
  static async get(): Promise<UserSettings> {
    let x = await appStore.get<object>(AppStoreKey.UserSetttings);
    if (!x) {
      x = {};
    }
    let r = new UserSettings();
    for (const key in x) {
      const v = x[key];
      if (v) {
        r[key] = v;
      }
    }
    return r;
  }
  static async set(x: object): Promise<void> {
    let old = await UserSettings.get();
    let s = { ...old, ...x };

    return appStore.set(AppStoreKey.UserSetttings, s);
  }
}

const SettingContext = createContext<UserSettings>(null);
const windowFocusChangeEvent = "window-focus-change-event";
const WindowFocusChangeContext = createContext(true);

function editorSetPositonAndReveal(editor: editor.IStandaloneCodeEditor, pos: Position) {
  editor.setPosition(pos);
  editor.focus();
  editor.revealLines(pos.lineNumber, pos.lineNumber);

}
const maxTreeSize = 50;

function postgresLike(x: DataSourceKind) {
  switch (x) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      return true
    default: return false;
  }
}
const udfIdGenerate = (
  kind: DataSourceKind,
  named: types.NamedStaff,
  uuid: string,
  typ: string,
) => {
  let idPrefix = `${uuid} + ${typ} + ${named.name}-`;
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      return (
        idPrefix +
        '-' +
        named.name +
        '-' +
        (named.parameters ? named.parameters : '')
      );
    case DataSourceKind.Cassandra:
      return (
        idPrefix + '-' + named.name + '-' + named.cassandra_parameters.join('-')
      );
    default:
      return idPrefix + '-' + named.name;
  }
};

function getEditorText(
  editor: editor.IStandaloneCodeEditor,
  showNotification
) {
  const range = editor.getSelection();
  let text: string;
  if (range) {
    text = editor.getModel().getValueInRange(range);
  }
  if (!text) {
    text = editor.getValue();
  }
  text = text.trim();
  if (!text) {
    showNotification('error', 'empty statement');
    return undefined;
  }
  return text;
};

function makeGetParserErrr(uuid) {
  return (sql: string): Promise<ErrAndLoc | null> => {
    return invoke<ErrAndLoc | null>('sql_parse_error', {
      name: uuid,
      sql,
    });
  };
}
function makeExecuteSql(uuid) {
  return (sql: string): Promise<any> => {
    return invoke<ErrAndLoc | null>('execute_sql_no_open_conn', {
      name: uuid,
      sql,
    });
  };
}

export {
  TabsSetContext,
  ConnTypeContext,
  UuidContext,
  ContextMenuContext,
  TabContent,
  generateRandomString,
  hexTryToString,
  hexToString,
  stringToHex,
  backendCompletionItemToEditor,
  extractConfigNameFromUuid,
  calMenuStyle, chunkArray,
  errToString,
  getViewportSize,
  isTypeLargeText,
  DataSourceKind,
  connTypeToDataSourceKind,
  secondConfirm,
  appStore,
  AppStoreKey,
  columnWidths,
  handleCopyToClipboard,
  validJson,
  uint8ArrayToUtf8,
  VisiableContext,
  FavoriteSql,
  MatchPatternContext,
  resetMuiDataRespWidth,
  quoteStyle,
  saveAndSetSize,
  SaveSizesToStore,
  DocsAndCount,
  MongoTableHandler,
  UserSettings,
  SettingContext,
  SetCloseConfirmTextContext,
  windowFocusChangeEvent,
  WindowFocusChangeContext,
  gridStyle,
  editorSetPositonAndReveal,
  maxTreeSize,
  postgresLike,
  udfIdGenerate,
  getEditorText,
  makeGetParserErrr,
  makeExecuteSql
};

export type { ErrAndLoc }

