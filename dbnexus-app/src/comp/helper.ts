'use strict';
import { editor, languages, Position } from "monaco-editor";
import types from "./ApiStruct";
import { createContext, CSSProperties, MouseEvent, ReactElement, ReactNode } from "react";
import { ask } from '@tauri-apps/plugin-dialog';
import { load } from "@tauri-apps/plugin-store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import { BSON, EJSON } from "bson";
import Database, { QueryResult } from '@tauri-apps/plugin-sql';
import { fetch } from '@tauri-apps/plugin-http';
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { emit } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";


const pageSizeOptions = [10, 20, 50, 100];
const appStore = await load('store.json');


const appDatabase = await Database.load('sqlite:dbpal.sqlite.db');
function setUpDatabase() {
  const sqls = [`
  CREATE TABLE if not EXISTS query_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL, -- uuid like mysql/xxx
    name text not null,  -- name like mysql
    sql text NOT NULL,
    create_at DATETIME NOT NULL DEFAULT current_timestamp,
    success integer NOT NULL
  );` ,
    `CREATE INDEX if not EXISTS index_query_log_uuid ON query_log (uuid);`,
    `
  CREATE TABLE if not EXISTS element_size ( 
    name TEXT PRIMARY KEY not NULL,
    width INTEGER NULL,
    height INTEGER NULL
  )
    `  , `
  CREATE TABLE if not EXISTS column_width ( 
    name TEXT PRIMARY KEY not NULL,
    width INTEGER NOT NULL
  )
    `  , `
  CREATE table if NOT EXISTS favorite_sql (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT not NULL,
    kind text not NULL,
    sql text not NULL,
    name text not NULL,
    connection_name text not NULL,
    last_modified datetime not NULL DEFAULT current_timestamp
  )
    ` , `
      CREATE INDEX if not EXISTS index_favorite_sql_uuid ON favorite_sql (uuid);
    ` ];
  sqls.forEach(async (sql) => {
    try { await appDatabase.execute(sql) } catch (e) {
      console.log("apply sql failed,this maybe a error", e)
    }
  })
}

setUpDatabase();

function rustResultOk(x: any): boolean {
  if (x instanceof Array) {
    for (let i = 0; i < x.length; i++) {
      if (!rustResultOk(x[i])) {
        return false;
      }
    }
    return true;
  }
  return !!x['Ok'];
}

class QueryLogHandler {
  uuid: string
  name: string
  constructor(uuid: string) {
    this.uuid = uuid;
    this.name = extractConfigNameFromUuid(uuid);
  }
  async save(sql: string, success: boolean, showNotification?): Promise<void> {
    return appDatabase.execute("insert into query_log (uuid,name,sql,success) values (?,?,?,?)", [this.uuid, this.name, sql, success ? 1 : 0]).then(() => { }).catch(e => {
      if (showNotification) { showNotification("error", "save query log failed", e); }

    });
  }

  query(matchSql: string, page: number, pageSize: number, order_by: string): Promise<any[]> {
    let sql = `
    select * from query_log where uuid=? ${matchSql ? `and sql like '%${matchSql}%'` : ""} order by ${order_by ? order_by : 'create_at desc'} limit ${pageSize} offset ${pageSize * (page - 1)}`;
    console.log("query log sql:", sql);
    return appDatabase.select(sql, [this.uuid])
  }
  queryCount(matchSql: string): Promise<number> {
    let sql = `select count(*) as total from query_log where uuid=? ${matchSql ? `and sql like '%${matchSql}%'` : ""}`;
    return appDatabase.select(sql, [this.uuid]).then(e => e ? e[0].total : 0);
  }
  delete(): Promise<QueryResult> {
    return appDatabase.execute("delete from query_log where uuid=?", [this.uuid]);
  }
  static deleteByConnectionname(name): Promise<QueryResult> {
    return appDatabase.execute("delete from query_log where name=?", [name]);
  }
}


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

function hexTryToString(hex): string | null {
  if (!hex) {
    return '';
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(
      new Uint8Array(bytes)
    );
  } catch {
    return null;
  }
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
      case 'Domain':
        return languages.CompletionItemKind.Operator;
      default:
        return languages.CompletionItemKind.Text;
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


const menuDivID = "menu-div";
function calMenuStyle(e: MouseEvent): CSSProperties {
  e.preventDefault();
  const x = e.clientX;
  const y = e.clientY;
  let ret: CSSProperties = {};
  const menuDiv = document.getElementById(menuDivID);
  menuDiv.style.display = 'block';
  const { height, width } = getViewportSize();
  ret.top = 'auto';
  ret.bottom = 'auto';
  ret.left = 'auto';
  ret.right = 'auto';
  if (width - x > (width / 2)) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      'px';
  } else {
    ret.right = width - x - 10 + 'px';
  }

  if ((height - y) > (height / 2)) {
    // 这个是点击的位置下面有很大的空间
    ret.top = y - 10 + 'px';
  } else {
    ret.bottom = height - y - 10 + 'px';
  }
  return ret;
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

function quoteIdentifier(x: DataSourceKind) {

  return (identifier: string) => {
    let q = "";
    switch (x) {
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.ClickHouse:
        q = '`';
        break;
      default:
        q = '"';
        break;
    }
    return q + identifier + q;
  }

}

function connTypeToDataSourceKind(name: string): DataSourceKind {
  return name as DataSourceKind
}
function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  return toUpper.includes('TEXT') || toUpper.includes("LONG") || toUpper.includes("CLOB")
}


enum AppStoreKey {
  UserSetting = 'user-settings',
  ConnectionConfig = 'connection-config',
  Zoom = "zoom",
  RedisShowDoc = "redis-show-doc",
  PageSize = "page-size",
  Align = "align",
  MatchPattern = "match-pattern",
  LeftTopShow = "left-top-show",
}


class SaveLeftTopShow {
  leftTopShow = true;
  get() {
    return this.leftTopShow;
  }
  save(x: boolean) {
    this.leftTopShow = x;
    appStore.set(AppStoreKey.LeftTopShow, this.leftTopShow)
  }
  static instance = new SaveLeftTopShow();
}
{
  let x = await appStore.get(AppStoreKey.LeftTopShow);
  if (typeof x == 'boolean') {
    SaveLeftTopShow.instance.leftTopShow = x;
  }
}


class SaveMatchPattern {
  matchPattern = [];

  get() {
    return this.matchPattern;
  }
  save(x: string[]) {
    this.matchPattern = x;
    appStore.set(AppStoreKey.MatchPattern, this.matchPattern);
  }
  static instance = new SaveMatchPattern();
}
{
  let x = await appStore.get<string[]>(AppStoreKey.MatchPattern);
  if (x) {
    SaveMatchPattern.instance.matchPattern = x;
  }
}


class SaveAlign {
  align = "center";
  constructor() {
    appStore.get<string>(AppStoreKey.Align).then(x => {
      if (x) {
        this.align = x;
      }
    });
  }

  get() {
    return this.align;
  }
  save(a: string) {
    this.align = a;
    appStore.set(AppStoreKey.Align, this.align);
  }
  static instance = new SaveAlign();
}

class SaveDomSizes {
  static async saveSize(key: string, size: UISize) {
    return appDatabase.execute("replace into element_size(name, width, height) values(?,?,?)", [key, Math.ceil(size.width), Math.ceil(size.height)]);
  }
  static async getSize(key: string): Promise<undefined | UISize> {
    return appDatabase.select("select width, height from element_size where name = ?", [key]).then((xs) => {
      if (xs instanceof Array) { return xs[0] }
      return xs
    })
  }
}


class FavoriteSql {
  constructor(public uuid: string, public name: string, public kind: DataSourceKind, public sql: string, public last_modified: Date = new Date(), public id?: number, public connection_name?: string) {
    this.connection_name = extractConfigNameFromUuid(uuid);
  }
  static async save(x: FavoriteSql, showNotification) {
    try {
      await appDatabase.execute("insert into favorite_sql(uuid,connection_name,name,kind,sql,last_modified) values(?,?,?,?,?,?)", [x.uuid, x.connection_name, x.name, x.kind, x.sql, x.last_modified]);
      showNotification('success', "save favorite sql Ok");
    } catch (e) {
      showNotification('error', "save favorite sql failed", e);
      return
    }
  }

  static async getDistinctUuid() {
    return appDatabase.select<{ uuid: string }[]>("SELECT DISTINCT  uuid from favorite_sql").then(x => x.map(y => y.uuid))
  }

  // todo performance issue????
  static getByUuid(uuid: string): Promise<FavoriteSql[]> {
    let sql = "select * from favorite_sql";
    if (uuid) {
      sql += " where uuid=?";
    }
    return appDatabase.select(sql, uuid ? [uuid] : []);
  }


  static async delete(id: number, showNotification) {
    try {
      await appDatabase.execute("delete from favorite_sql where id=?", [id]);
    } catch (e) {
      showNotification('error', "save favorite sql failed", e);
      return
    }
    showNotification('success', "delete sql Ok")
  }

  static async deleteByConnectionName(name: string) {
    try {
      appDatabase.execute(`delete from favorite_sql where connection_name=?`, [name]);
    } catch (e) {
      console.log("delete all logs belong to a key failed", e);
      return
    }
  }
  static async clear(showNotification) {
    try {
      appDatabase.execute(`delete from favorite_sql`);
    } catch (e) {
      showNotification('error', "clear favorite sql failed", e);
      return
    }
    showNotification('success', "clean OK");
  }
}

class ColumnWidth {
  width = {};
  constructor() {
    appDatabase.select("select * from column_width").then((xs) => {
      (xs as { name: string, width: number }[]).forEach(e => {
        this.width[e.name] = e.width
      })
    })
  }
  save(name: string, width: number) {
    this.width[name] = width;
    appDatabase.execute("replace into column_width(name,width) values(?,?)", [name, Math.ceil(width)])
  }

  get(key): number {
    return this.width[key];
  }
  reset(): Promise<void> {
    this.width = {};
    return appDatabase.execute("delete from column_width").then((x) => {
      console.log("delete of", x.rowsAffected, "rows");
    })

  }
  static instance = new ColumnWidth();
}

const TabActiveContext = createContext(true);
const handleCopyToClipboard = (text, showNotification?, okCallBack?) => {
  writeText(text)
    .then(() => {
      if (showNotification) { showNotification('success', 'success copy to clipboard.'); }
      if (okCallBack) { okCallBack() }
    })
    .catch((e) => {
      if (showNotification) {
        showNotification('error', 'copy to clipboard failed', e);
      }
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

function resetMuiDataRespWidth(old: MuiDataGridData): MuiDataGridData {
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
const gridStyle = { height: "auto", maxHeight: "80vh" } as CSSProperties;

function saveAndSetSize(dom: Element, key: string, set: "height" | "width" | "both" = "both") {
  let e = new ResizeObserver((en) => {
    const height = en[0].target.clientHeight;
    const width = en[0].target.clientWidth;
    if (width && height) {
      SaveDomSizes.saveSize(key, { width, height });
    }
  });
  SaveDomSizes.getSize(key).then(size => {
    if (size) {
      let div = (dom as HTMLDivElement);
      if (set == "height" || set == "both") {
        if (size.height) {
          div.style.height = size.height + 'px';
        }
      }
      if (set == "width" || set == "both") {
        if (size.width) {
          div.style.width = size.width + 'px';
        }
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
    return async (filter, sort, page, pageSize): Promise<MuiDataGridData> => {
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
      return invoke('mongo_show_collection_docs', {
        name: uuid,
        tableName: table,
        filter: BSON.serialize(filter),
        sort: BSON.serialize(sort),
        page,
        pageSize,
      }).then(x => {
        //@ts-ignore
        let docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
        //@ts-ignore
        let count = x.count;
        return new DocsAndCount(docs, count);
      });
    }
  }

  static loadOne(uuid: string, table) {
    return async (id): Promise<BSON.Document> => {
      return invoke<number[]>('mongo_get_one_document_by_id', {
        name: uuid,
        tableName: table,
        id: JSON.stringify(EJSON.serialize(id)),
      }).then(x => {
        return BSON.deserialize(new Uint8Array(x))
      });
    }
  }


  static modifyOP(uuid, table) {
    return async (doc) => {
      return invoke<void>('mongo_update_one', {
        name: uuid,
        tableName: table,
        doc: BSON.serialize(doc),
      });
    }
  }
  static deleteOP(uuid, table) {
    return async (ids) => {
      if (!Array.isArray(ids)) {
        ids = [ids];
      }
      return invoke<void>('mongo_delete_ids', {
        name: uuid,
        tableName: table,
        ids: JSON.stringify(EJSON.serialize(ids)),
      });
    }
  }
}

class UserSetting {
  theme: string = 'light';
  fontSize: number = 15;
  fontWeight: string = 'normal';
  fontFamily: string = undefined;
  tabSize: number = 4;
  lightBgColor: string = undefined;
  darkBgColor: string = undefined;
  csvDelimiter: string = ',';
  includeHeader: boolean = true;
  datagridRowHeightFactor = 2;
  maxTreeSize: number = 150;
  loggerLevel: string = 'info';
  loggerMaxSize: number = 1;
  saveQueryLog: boolean = true;
  maxTabTitleLength: number = 12;
  editorOption: editor.IStandaloneEditorConstructionOptions = {};
  editorTheme(): string {
    return this.isDark() ? 'vs-dark' : 'light';
  }

  isDark(): boolean {
    return this.theme === 'dark'
  }
  // 可能和以前的设置冲突
  datagridRowHeightXXX() {
    return Math.ceil(this.fontSize * this.datagridRowHeightFactor)
  }
  static async get(): Promise<UserSetting> {
    let x = await appStore.get<object>(AppStoreKey.UserSetting);
    if (!x) {
      x = {};
    }
    let r = new UserSetting();
    for (const key in x) {
      r[key] = x[key];
    }
    return r;
  }
  static async set(x: object): Promise<void> {
    let old = await UserSetting.get();
    let s = { ...old, ...x };

    return appStore.set(AppStoreKey.UserSetting, s);
  }
}

const SettingContext = createContext<UserSetting>(null);
const windowFocusChangeEvent = "window-focus-change-event";
const WindowFocusContext = createContext(true);

function editorSetPositionAndReveal(editor: editor.IStandaloneCodeEditor, pos: Position) {
  editor.setPosition(pos);
  editor.focus();
  editor.revealLines(pos.lineNumber, pos.lineNumber);

}


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

function makeGetParserErr(uuid) {
  return (sql: string): Promise<ErrAndLoc | null> => {
    return invoke<ErrAndLoc | null>('sql_parse_error', {
      name: uuid,
      sql,
    });
  };
}
function makeExecuteSql(uuid, saveQueryLog: boolean) {
  return (sql: string): Promise<any> => {
    return invoke<any>('execute_sql_no_open_conn', {
      name: uuid,
      sql,
    }).then(e => {
      if (saveQueryLog) {
        new QueryLogHandler(uuid,).save(sql, rustResultOk(e));
      }
      return e
    });
  };
}


function getStackTrace() {
  const error = new Error();
  console.log(error.stack);
}



class ColorFn {

  static hexToRgb(hex) {
    hex = hex.replace('#', '');
    if (hex.length === 3) hex = [...hex].map(c => c + c).join('');
    const num = parseInt(hex, 16);
    return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
  }
  static isLightHex(hex) {
    const [r, g, b] = ColorFn.hexToRgb(hex);
    const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
    return lum > 0.5;
  }
  static isColor = str => {
    const o = new Option().style;
    o.color = str;
    return o.color !== '';
  };
}


class SavePageSize {
  sizes = {}

  constructor() {
    appStore.get(AppStoreKey.PageSize).then((sizes) => {
      if (sizes) {
        this.sizes = sizes;
      }
    })
  }
  save(size: number, key = "default") {
    if (pageSizeOptions.find(e => size == e)) {
      this.sizes[key] = size;
      appStore.set(AppStoreKey.PageSize, this.sizes);
    } else {
      console.log("invalid page size", size, "this is bug");
      getStackTrace();
    }
  }
  get(key = "default", defaultSize = 100) {
    return this.sizes[key] || defaultSize;
  }
  static instance = new SavePageSize();
}


function getSupportExportKind(kind: DataSourceKind, isTable: boolean) {
  switch (kind) {
    case DataSourceKind.Mongo:
      return ['json', 'jsonline'];
    default:
      return isTable ? ['csv', 'sql', 'json', 'jsonline', "xlsx"] : ['csv', 'json', 'jsonline', "xlsx"];
  }
}
const validURL = async (url) => {
  let x = await fetch(url);
  return x.ok;
};


function generateDefaultFilename(name, extension) {
  if (extension == 'jsonline') {
    /*
      统一用 .jsonl（全小写，无空格）最好。
      这是社区事实标准：GitHub、Git LFS、Apache Spark、BigQuery、Hugging Face 等生态都按 .jsonl 识别，工具链（grep、jq、ripgrep、VS Code 插件）也都默认支持。
    */
    extension = 'jsonl';
  }
  // 获取当前时间
  const now = new Date();
  // 格式化时间为字符串
  const timestamp = now.toISOString().replace(/[:.-]/g, '').slice(0, 15);
  // 生成文件名
  const filename = `${name}_${timestamp}.${extension}`;
  return filename;
}
const textEllipsisStyle = {
  width: '9rem',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  flex: 1,
} as CSSProperties;


export type MuiDataGridData = {
  "fields": types.Field[];
  "data": any[];
  "sql": string | null;
  "total": number | null;
  "cache_time": types.U64 | null;
  "has_more": boolean;
};

async function setZoom(out: boolean) {
  const currentWebview = WebviewWindow.getCurrent();
  let zoom = await appStore.get<number>(AppStoreKey.Zoom);
  if (!zoom) {
    zoom = 1;
  }
  const min = 0.5;
  const max = 1.5;
  if (out) {
    zoom += 0.1;
    if (zoom > max) {
      zoom = max;
    }
  } else {
    zoom -= 0.1;
    if (zoom < min) {
      zoom = min;
    }
  }
  currentWebview.setZoom(zoom);
  await appStore.set(AppStoreKey.Zoom, zoom);
}



async function windowInit(isMain: boolean, emitFocus: boolean) {
  const currentWebview = await WebviewWindow.getCurrent();
  appStore.get<number>(AppStoreKey.Zoom).then(async (zoom) => {
    if (zoom) {
      await currentWebview.setZoom(zoom);
    }
  });

  if (['windows', 'linux'].includes(platform())) {
    document.addEventListener('keydown', function (event) {
      if (event.key === 'F11') {
        currentWebview.isFullscreen().then((isFull) => {
          currentWebview.setFullscreen(!isFull);
        });
      }
    });
  }
  if (emitFocus) {
    currentWebview.onFocusChanged((x) => {
      emit(windowFocusChangeEvent, x.payload);
    });
  }

  if (isMain) {
    if (!import.meta.env.DEV) {
      currentWebview.theme().then(async (theme) => {
        if (!theme) {
          return;
        }
        let set = await UserSetting.get();
        set.theme = theme;
        appStore.set(AppStoreKey.UserSetting, set);
      });
    }

    currentWebview.onThemeChanged(async (msg) => {
      let set = await UserSetting.get();
      set.theme = msg.payload;
      appStore.set(AppStoreKey.UserSetting, set);
    });
  }

  if (!import.meta.env.DEV) {
    document.addEventListener('contextmenu', function (event) {
      event.preventDefault();
    });

    document.addEventListener('keydown', function (event) {
      // Prevent F5 or Ctrl+R (Windows/Linux) and Command+R (Mac) from refreshing the page
      if (
        event.key === 'F5' ||
        (event.ctrlKey && event.key === 'r') ||
        (event.metaKey && event.key === 'r')
      ) {
        event.preventDefault();
      }
    });
  }
}


enum RedisTableType {
  String = 'string',
  List = 'list',
  Set = 'set',
  Zset = 'zset',
  Hash = 'hash',
}

function extractFieldAndIDFromMouseEvent(e: MouseEvent) {

  const { currentTarget, target } = e;
  let field = (target as HTMLDivElement).getAttribute('data-field');
  let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
  return { field, id };
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
  calMenuStyle,
  errToString,
  getViewportSize,
  isTypeLargeText,
  DataSourceKind,
  connTypeToDataSourceKind,
  secondConfirm,
  appStore,
  AppStoreKey,
  ColumnWidth,
  handleCopyToClipboard,
  validJson,
  TabActiveContext,
  FavoriteSql,
  MatchPatternContext,
  resetMuiDataRespWidth,
  quoteIdentifier,
  saveAndSetSize,
  SaveDomSizes,
  DocsAndCount,
  MongoTableHandler,
  UserSetting,
  SettingContext,
  SetCloseConfirmTextContext,
  windowFocusChangeEvent,
  WindowFocusContext,
  gridStyle,
  editorSetPositionAndReveal,
  postgresLike,
  udfIdGenerate,
  getEditorText,
  makeGetParserErr,
  makeExecuteSql,
  QueryLogHandler,
  ColorFn,
  getSupportExportKind,
  SavePageSize,
  rustResultOk,
  validURL,
  generateDefaultFilename,
  pageSizeOptions,
  textEllipsisStyle,
  SaveAlign,
  SaveMatchPattern,
  SaveLeftTopShow,
  setZoom,
  windowInit,
  RedisTableType,
  extractFieldAndIDFromMouseEvent,
  menuDivID,
};


export type { ErrAndLoc }

