import { editor, languages, Position } from "monaco-editor";
import types from "./ApiStruct";
import {
  createContext,
  CSSProperties,
  MouseEvent,
  ReactElement,
  ReactNode,
} from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { Store } from "@tauri-apps/plugin-store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import { BSON } from "bson";
import Database, { QueryResult } from "@tauri-apps/plugin-sql";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import themelist from "../../monaco-themes/themes/themelist.json";
import { loader } from "@monaco-editor/react";
import { emit } from "@tauri-apps/api/event";
import { PassWordManager } from "./Common";
import { ChatOpenAI } from "@langchain/openai";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import * as monaco from "monaco-editor";
import prettyMilliseconds from "pretty-ms";

loader.config({ monaco });

const pageSizeOptions = [10, 20, 50, 100];
const appStore = await Store.load("store.json");
function prettyMs(x: number) {
  return prettyMilliseconds(x);
}

let appDatabase: Database = await Database.load("sqlite:dbpal.sqlite.db");

appDatabase.select("select * from column_width").then(xs => {
  (xs as { name: string; width: number }[]).forEach(e => {
    ColumnWidth.instance.width[e.name] = e.width;
  });
});

function rustResultOk(x: any): boolean {
  if (x instanceof Array) {
    for (const e of x) {
      if (!rustResultOk(e)) {
        return false;
      }
    }
    return true;
  }
  return !!x["Ok"];
}

class QueryLogHandler {
  uuid: string;
  name: string;
  constructor(uuid: string) {
    this.uuid = uuid;
    this.name = extractConfigNameFromUuid(uuid);
  }
  async save(sql: string, success: boolean, language?: string): Promise<void> {
    return appDatabase
      .execute(
        "insert into query_log (uuid,name,sql,language,success) values (?,?,?,?,?)",
        [this.uuid, this.name, sql, language, success ? 1 : 0],
      )
      .then(() => {})
      .catch(e => {
        message("save query log failed,e" + String(e), { kind: "error" });
      });
  }

  async query(
    matchSql: string,
    page: number,
    pageSize: number,
    order_by: string,
  ): Promise<any[]> {
    const sql = `
    select * from query_log where uuid=? ${
      matchSql ? `and sql like '%${matchSql}%'` : ""
    } order by ${order_by ? order_by : "create_at desc"} limit ${pageSize} offset ${
      pageSize * (page - 1)
    }`;
    console.info("query log sql:", sql);
    return appDatabase.select(sql, [this.uuid]);
  }
  async queryCount(matchSql: string): Promise<number> {
    const sql = `select count(*) as total from query_log where uuid=? ${
      matchSql ? `and sql like '%${matchSql}%'` : ""
    }`;
    return appDatabase.select(sql, [this.uuid]).then(e => (e ? e[0].total : 0));
  }
  async delete(): Promise<QueryResult> {
    return appDatabase.execute("delete from query_log where uuid=?", [this.uuid]);
  }
  static async deleteByConnectionName(name): Promise<QueryResult> {
    return appDatabase.execute("delete from query_log where name=?", [name]);
  }
}

interface ConfirmOptions {
  title: string;
  content?: string;
  confirmText?: string;
  cancelText?: string;
  kind?: "info" | "warning" | "error";
}

async function secondConfirm({
  title,
  content,
  confirmText = "Confirm",
  cancelText = "Cancel",
  kind = "warning",
}: ConfirmOptions): Promise<boolean> {
  return ask(title, {
    title: content,
    kind,
    okLabel: confirmText,
    cancelLabel: cancelText,
  });
}

function generateRandomString(length: number = 10): string {
  const characters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
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
    return "";
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(bytes));
  } catch {
    return null;
  }
}

function hexToString(hex: string): string {
  if (!hex) {
    return "";
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  return new TextDecoder("utf-8").decode(new Uint8Array(bytes));
}

function stringToHexArray(str: string): Uint8Array {
  // 创建一个 Uint8Array 来存储字节
  const bytes = new TextEncoder().encode(str);
  return bytes;
}

function arrayToHex(uint8Array: Uint8Array | number[]) {
  return Array.from(uint8Array) // 将 Uint8Array 转换为普通数组
    .map(byte => byte.toString(16).padStart(2, "0")) // 转换每个字节为十六进制并补齐
    .join(""); // 连接为一个字符串
}

function stringToHex(str): string {
  return arrayToHex(stringToHexArray(str));
}

function backendCompletionItemToEditor(
  xs: types.CompletionItem[],
): languages.CompletionList {
  function getKind(kind: types.CompletionKind): languages.CompletionItemKind {
    switch (kind) {
      case "Command":
        return languages.CompletionItemKind.Keyword;
      case "KeyWord":
        return languages.CompletionItemKind.Keyword;
      case "Function":
        return languages.CompletionItemKind.Function;
      case "Database":
        return languages.CompletionItemKind.Module;
      case "Schema":
        return languages.CompletionItemKind.Folder;
      case "Table":
      case "View":
        return languages.CompletionItemKind.Reference;
      case "Column":
        return languages.CompletionItemKind.Field;
      case "Text":
        return languages.CompletionItemKind.Text;
      case "Type":
        return languages.CompletionItemKind.Class;
      case "Index":
        return languages.CompletionItemKind.Reference;
      case "Sequence":
        return languages.CompletionItemKind.Reference;
      case "Stage":
        return languages.CompletionItemKind.Reference;
      case "Trigger":
        return languages.CompletionItemKind.Function;
      case "Event":
        return languages.CompletionItemKind.Event;
      case "Procedure":
        return languages.CompletionItemKind.Function;
      case "Extension":
        return languages.CompletionItemKind.Module;
      case "User":
        return languages.CompletionItemKind.User;
      case "Role":
        return languages.CompletionItemKind.User;
      case "Constraint":
        return languages.CompletionItemKind.Operator;
      case "Partition":
        return languages.CompletionItemKind.Operator;
      case "PrepareStatement":
        return languages.CompletionItemKind.Function;
      case "Pragma":
        return languages.CompletionItemKind.Keyword;
      case "Policy":
        return languages.CompletionItemKind.Operator;
      case "Variable":
        return languages.CompletionItemKind.Variable;
      case "Rule":
        return languages.CompletionItemKind.Operator;
      case "ClickHouseTableProjection":
        return languages.CompletionItemKind.Property;
      case "Prepare":
        return languages.CompletionItemKind.Function;
      case "Module":
        return languages.CompletionItemKind.Module;
      case "Domain":
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
        documentation: e.desc,
      };
    }),
  };
}

function extractConfigNameFromUuid(uuid: string): string {
  if (uuid.includes("/")) {
    return uuid.substring(0, uuid.indexOf("/"));
  } else {
    return uuid;
  }
}

function extractLastNameFromUuid(uuid: string) {
  if (uuid.includes("/")) {
    return uuid.substring(uuid.indexOf("/") + 1);
  } else {
    return uuid;
  }
}

function getViewportSize() {
  const width = window.innerWidth; // 获取视口的宽度
  const height = window.innerHeight; // 获取视口的高度

  return { width, height }; // 返回一个对象，包含宽度和高度
}

function errToString(message) {
  if (typeof message == "object" && "reason" in message) {
    // TODO show message
    message = message["reason"];
  }
  if (typeof message == "object" && message instanceof Error) {
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
  SqlServer = "sqlserver",
  Redis = "redis",
  Mongo = "mongo",
  Oracle = "oracle",
  Cassandra = "cassandra",
  ClickHouse = "clickhouse",
  LibSql = "libsql",
  FireBird = "firebird",
  DuckDB = "duckdb",
  Cockroach = "cockroach",
  Db2 = "db2",
}

function quoteDatabaseIdentifier(x: DataSourceKind) {
  return (identifier: string) => {
    let q = "";
    switch (x) {
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.ClickHouse:
        q = "`";
        break;
      default:
        q = '"';
        break;
    }
    return q + identifier + q;
  };
}

function connTypeToDataSourceKind(name: string): DataSourceKind {
  return name as DataSourceKind;
}
function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  return (
    toUpper.includes("TEXT") || toUpper.includes("LONG") || toUpper.includes("CLOB")
  );
}

class SaveLeftTopShow {
  key = "dbpal-left-top-show";
  get() {
    return localStorage.getItem(this.key) == "1";
  }
  save(x: boolean) {
    localStorage.setItem(this.key, x ? "1" : "0");
  }
  static instance = new SaveLeftTopShow();
}

class SaveMatchPattern {
  key = "dbpal-match-pattern";
  get() {
    try {
      const x = JSON.parse(localStorage.getItem(this.key));
      if (x instanceof Array) {
        return x;
      } else {
        return [];
      }
    } catch (e) {
      console.log("get match pattern failed", e);
      return [];
    }
  }
  save(x: string[]) {
    localStorage.setItem(this.key, JSON.stringify(x));
  }
  static instance = new SaveMatchPattern();
}

class SaveAlign {
  align = "center";
  key = "dbpal-align";

  get() {
    const r = localStorage.getItem(this.key);
    if (r) {
      return r;
    } else {
      return "center";
    }
  }
  save(a: string) {
    localStorage.setItem(this.key, a);
  }
  static instance = new SaveAlign();
}

class SaveDomSizes {
  static async saveSize(key: string, size: UISize) {
    return appDatabase.execute(
      "replace into element_size(name, width, height) values(?,?,?)",
      [key, Math.ceil(size.width), Math.ceil(size.height)],
    );
  }
  static async getSize(key: string): Promise<undefined | UISize> {
    return appDatabase
      .select("select width, height from element_size where name = ?", [key])
      .then(xs => {
        if (xs instanceof Array) {
          return xs[0];
        }
        return xs;
      });
  }
}

class FavoriteSql {
  constructor(
    public uuid: string,
    public name: string,
    public kind: DataSourceKind,
    public sql: string,
    public language?: string,
    public last_modified: Date = new Date(),
    public id?: number,
    public connection_name?: string,
  ) {
    this.connection_name = extractConfigNameFromUuid(uuid);
  }
  async save(showNotification) {
    try {
      await appDatabase.execute(
        "replace into favorite_sql(uuid,connection_name,name,kind,sql,language,last_modified) values(?,?,?,?,?,?,?)",
        [
          this.uuid,
          this.connection_name,
          this.name,
          this.kind,
          this.sql,
          this.language,
          this.last_modified,
        ],
      );
      showNotification("success", "save favorite sql Ok");
    } catch (e) {
      showNotification("error", "save favorite sql failed", e);
      return;
    }
  }

  static async getDistinctUuid() {
    return appDatabase
      .select<{ uuid: string }[]>("SELECT DISTINCT  uuid from favorite_sql")
      .then(x => x.map(y => y.uuid));
  }

  // todo performance issue????
  static async getByUuid(uuid: string): Promise<FavoriteSql[]> {
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
      showNotification("error", "save favorite sql failed", e);
      return;
    }
    showNotification("success", "delete sql Ok");
  }

  static async deleteByConnectionName(name: string) {
    try {
      appDatabase.execute(`delete from favorite_sql where connection_name=?`, [name]);
    } catch (e) {
      console.error("delete all logs belong to a key failed", e);
      return;
    }
  }

  static async clearByUuid(showNotification, uuid: string) {
    try {
      appDatabase.execute(`delete from favorite_sql where uuid=?`, [uuid]);
    } catch (e) {
      showNotification("error", "clear favorite sql failed", e);
      return;
    }
    showNotification("success", "clean OK");
  }
}

class ColumnWidth {
  width = {};
  constructor() {}
  async save(name: string, width: number) {
    this.width[name] = width;
    appDatabase.execute("replace into column_width(name,width) values(?,?)", [
      name,
      Math.ceil(width),
    ]);
  }

  get(key): number {
    return this.width[key];
  }
  async reset(): Promise<void> {
    this.width = {};
    return appDatabase.execute("delete from column_width").then(x => {
      console.info("delete of", x.rowsAffected, "rows");
    });
  }
  static instance = new ColumnWidth();
}

const TabActiveContext = createContext(true);
const handleCopyToClipboard = (text, showNotification?, okCallBack?) => {
  writeText(text)
    .then(() => {
      if (showNotification) {
        showNotification("success", "success copy to clipboard.");
      }
      if (okCallBack) {
        okCallBack();
      }
    })
    .catch(e => {
      if (showNotification) {
        showNotification("warning", "copy to clipboard failed", e);
      }
    });
};

class TabContent {
  constructor(
    public id: string,
    public header: string,
    public content: ReactNode,
    public icon: ReactElement,
    public closeConfirmText?: string,
  ) {}
}
const TabsSetContext = createContext<{
  tabs: TabContent[];
  setTabs;
  setSelectTab;
}>(null);

const LLMConfigContext = createContext<null | LLMConfig>(null);

const SetCloseConfirmTextContext = createContext<(x: string) => void>(null);

const ConnTypeContext = createContext<{
  kind: DataSourceKind;
}>(null);

const MatchPatternContext = createContext<(x: string) => boolean>(() => {
  return true;
});

const UuidContext = createContext<{
  uuid: string;
}>(null);

const ContextMenuContext = createContext<{
  setUpMenu: ({ content, event }) => void;
}>(null);

interface ErrAndLoc {
  line?: number;
  column?: number;
  msg: string;
}

function validJson(s: string): Promise<ErrAndLoc | null> {
  return invoke<ErrAndLoc | null>("valid_json", { jsonStr: s });
}

type UISize = {
  width?: number;
  height?: number;
};

function saveAndSetSize(
  dom: Element,
  key: string,
  set: "height" | "width" | "both" = "both",
): () => void {
  const e = new ResizeObserver(en => {
    const height = en[0].target.clientHeight;
    const width = en[0].target.clientWidth;
    if (width && height) {
      SaveDomSizes.saveSize(key, { width, height });
    }
  });
  SaveDomSizes.getSize(key).then(size => {
    if (size) {
      const div = dom as HTMLDivElement;
      if (set == "height" || set == "both") {
        if (size.height) {
          div.style.height = size.height + "px";
        }
      }
      if (set == "width" || set == "both") {
        if (size.width) {
          div.style.width = size.width + "px";
        }
      }
    }
  });
  e.observe(dom);
  return () => {
    e.disconnect();
  };
}

class DocsAndCount {
  constructor(public docs: BSON.Document[], public count: number) {}
}

function notifySettingChange() {
  emit(settingChangeEvent, "");
}

class UserSettingAppearance {
  fontSize: number = 15;
  fontWeight: string = "normal";
  fontFamily: string = "";
  lightBgColor: string = "";
  darkBgColor: string = "";
  datagridRowHeightFactor = 2.5;
  intputVariant: string = "standard";
  static key = "dbpal-appearance-setting";
  save() {
    localStorage.setItem(UserSettingAppearance.key, JSON.stringify({ ...this }));
    notifySettingChange();
  }
  // 可能和以前的设置冲突
  datagridRowHeight() {
    return Math.ceil(this.fontSize * this.datagridRowHeightFactor);
  }
  static get(): UserSettingAppearance {
    try {
      let setting = JSON.parse(localStorage.getItem(UserSettingAppearance.key));
      setting = { ...new UserSettingAppearance(), ...setting };
      Object.setPrototypeOf(setting, UserSettingAppearance.prototype);
      return setting;
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingAppearance();
    }
  }
}

class UserSettingExport {
  csvDelimiter: string = ",";
  includeHeader: boolean = true;
  static key = "dbpal-export-setting";
  save() {
    localStorage.setItem(UserSettingExport.key, JSON.stringify({ ...this }));
    notifySettingChange();
  }
  static get(): UserSettingExport {
    try {
      let setting = JSON.parse(localStorage.getItem(UserSettingExport.key));
      setting = { ...new UserSettingExport(), ...setting };
      Object.setPrototypeOf(setting, UserSettingExport.prototype);
      return setting;
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingExport();
    }
  }
}

class UserSettingEditor {
  tabSize: number = 4;
  theme: string = "";
  editorOption: any = null;
  static key = "dbpal-editor-setting";
  save() {
    localStorage.setItem(UserSettingEditor.key, JSON.stringify({ ...this }));
    notifySettingChange();
  }

  static get() {
    try {
      const x = JSON.parse(localStorage.getItem(UserSettingEditor.key));
      return { ...new UserSettingEditor(), ...x };
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingEditor();
    }
  }
}

class UserSetting {
  editor: UserSettingEditor = UserSettingEditor.get();
  appearance: UserSettingAppearance = UserSettingAppearance.get();
  export: UserSettingExport = UserSettingExport.get();
  misc: UserSettingMisc = UserSettingMisc.get();
  editorTheme(): string {
    if (this.editor.theme) {
      return this.editor.theme;
    }
    return isDarkTheme() ? "vs-dark" : "light";
  }
  datagridRowHeightXXX() {
    return this.appearance.datagridRowHeight();
  }
}

let appTheme = "light";

function isDarkTheme() {
  return appTheme == "dark";
}

class UserSettingMisc {
  static key = "dbpal-misc-setting";
  autoStart: boolean = false;
  save() {
    localStorage.setItem(UserSettingMisc.key, JSON.stringify({ ...this }));
    notifySettingChange();
  }

  static get() {
    try {
      const x = JSON.parse(localStorage.getItem(UserSettingMisc.key));
      return { ...new UserSettingMisc(), ...x };
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingMisc();
    }
  }
}

const SettingContext = createContext<UserSetting>(null);

const WindowFocusContext = createContext(true);

function editorSetPositionAndReveal(
  editor: editor.IStandaloneCodeEditor,
  pos: Position,
) {
  editor.setPosition(pos);
  editor.focus();
  editor.revealLines(pos.lineNumber, pos.lineNumber);
}

class LoggerLevel {
  static key: string = "logger-level";

  static level: string = "none";
  static async set(level: string) {
    LoggerLevel.level = level;
    await appStore.set(LoggerLevel.key, level);
  }
}
{
  appStore.get<string>(LoggerLevel.key).then(level => {
    if (level) {
      LoggerLevel.level = level;
    }
  });
}

function postgresLike(x: DataSourceKind) {
  switch (x) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      return true;
    default:
      return false;
  }
}

function getEditorText(editor: editor.IStandaloneCodeEditor, showNotification) {
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
    showNotification("warning", "empty statement");
    return undefined;
  }
  return text;
}

class SaveQueryLogConfigure {
  static key = "dbpal-save-query-log";
  static get() {
    try {
      return "1" == localStorage.getItem(SaveQueryLogConfigure.key);
    } catch (e) {
      console.error("get query log failed", e);
      return false;
    }
  }
  static set(save: boolean) {
    localStorage.setItem(SaveQueryLogConfigure.key, save ? "1" : "0");
  }
}

function executeSqlWithoutConn(uuid: string, sql: string) {
  return invoke<QueryRespData>("execute_sql_no_open_conn", {
    name: uuid,
    sql,
  }).then(e => {
    if (SaveQueryLogConfigure.get()) {
      new QueryLogHandler(uuid).save(sql, rustResultOk(e));
    }
    return e;
  });
}

function getStackTrace() {
  const error = new Error();
  console.error(error.stack);
}

class ColorFn {
  static hexToRgb(hex) {
    hex = hex.replace("#", "");
    if (hex.length === 3) hex = [...hex].map(c => c + c).join("");
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
    return o.color !== "";
  };
}

class SavePageSize {
  key = "dbpal-page-size";

  save(size: number) {
    if (pageSizeOptions.find(e => size == e)) {
      localStorage.setItem(this.key, size + "");
    } else {
      getStackTrace();
    }
  }

  get() {
    try {
      const x = parseInt(localStorage.getItem(this.key));
      if (!isNaN(x)) {
        return x;
      }
    } catch (e) {
      console.log("get page size failed", e);
      console.error("get page size failed");
    }
    return 100;
  }
  static instance = new SavePageSize();
}

function getSupportExportKind(kind: DataSourceKind, isTable: boolean) {
  switch (kind) {
    case DataSourceKind.Mongo:
      return ["json", "jsonline"];
    default:
      return isTable
        ? ["csv", "sql", "json", "jsonline", "xlsx", "html", "markdown"]
        : ["csv", "json", "jsonline", "xlsx", "html", "markdown"];
  }
}

function generateDefaultFilename(name, extension) {
  if (extension == "jsonline") {
    /*
      统一用 .jsonl（全小写，无空格）最好。
      这是社区事实标准：GitHub、Git LFS、Apache Spark、BigQuery、Hugging Face 等生态都按 .jsonl 识别，工具链（grep、jq、ripgrep、VS Code 插件）也都默认支持。
    */
    extension = "jsonl";
  }
  if (extension == "markdown") {
    extension = "md";
  }
  // 获取当前时间
  const now = new Date();
  // 格式化时间为字符串
  const timestamp = now.toISOString().replace(/[:.-]/g, "").slice(0, 15);
  // 生成文件名
  const filename = `${name}_${timestamp}.${extension}`;
  return filename;
}
const textEllipsisStyle = {
  width: "9rem",
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
  flex: 1,
} as CSSProperties;

export type MuiDataGridData = {
  fields: types.Field[];
  data: any[];
  sql: string | null;
  total: number | null;
  cache_time: types.U64 | null;
  has_more: boolean;
};

enum RedisTableType {
  String = "string",
  List = "list",
  Set = "set",
  Zset = "zset",
  Hash = "hash",
}

function extractFieldAndIDFromMouseEvent(e: MouseEvent) {
  const { currentTarget, target } = e;
  const field = (target as HTMLDivElement).getAttribute("data-field");
  const id = (currentTarget as HTMLDivElement).getAttribute("data-id");
  return { field, id };
}

function editorPositionToEnd(editor: editor.IStandaloneCodeEditor) {
  const model = editor.getModel();
  const lastLine = model.getLineCount();
  const lastColumn = model.getLineMaxColumn(lastLine);
  // 设置光标位置到最后一行最后一列
  editor.setPosition({ lineNumber: lastLine, column: lastColumn });
}

class KeyPressed {
  // control on windows on command on mac pressed and expect key
  static matchControl(e: KeyboardEvent, expect: string) {
    return (
      (isMacOs() && e.metaKey && e.key == expect) ||
      (!isMacOs() && e.ctrlKey && e.key == expect)
    );
  }

  // is is control+s pressed
  static isFileSave(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "s");
  }

  static isFileOpen(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "o");
  }

  static prevTab(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "[");
  }
  static nextTab(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "]");
  }
  static closeTab(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "w");
  }
}

function isMacOs() {
  return platform() == "macos";
}

function isWindows() {
  return platform() == "windows";
}

const dataGridStyle = {
  flexGrow: 1,
  height: "100px",
  flexShrink: 1,
  // 这个值用 0% 在windows也有问题好像会把datagrid变得很高 用0 都可以 用0% 都不可以 然而flex为1 设置的值是0% 可能是兼容性的问题
  flexBasis: "0",
} as CSSProperties;

enum ViewKind {
  View,
  MaterializedView,
}

const registeredThemes = [];

async function setUpEditorThemes() {
  try {
    const themes = await invoke<object>("monaco_editor_themes");
    const monaco = await loader.init();
    for (const key in themes) {
      themes[key] = JSON.parse(themes[key]);
    }
    for (const key in themelist) {
      const v = themelist[key];
      monaco.editor.defineTheme(key, themes[v]);
      registeredThemes.push(key);
    }
  } catch (e) {
    message("set up editor themes failed,err:" + String(e), { kind: "error" });
    console.error("set up editor themes failed", e);
  }
}

setUpEditorThemes();
function getLanguageFromDataSourceKind(kind: DataSourceKind): string {
  switch (kind) {
    case DataSourceKind.Redis:
      return "redis";
    case DataSourceKind.Mongo:
      return "javascript";
    default:
      return "sql";
  }
}

const settingChangeEvent = "setting-change";
const dataGridheaderFontSize = "0.8rem";

enum TableType {
  Normal = "normal",
  MongoTimeSerial = "time-serial",
}

const shrinkAble = {
  flexShrink: 1,
  overflow: "hidden",
  minWidth: "3rem",
};
class LLMConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
  temperature: number = 0;
  static key = "llm-config-4k9zF7m5p3T1H2";
  maxToken: number = 0;
  topP: number = 0;
  aiTextTemplates: any = {};
  createChatAPI() {
    const llm = new ChatOpenAI({
      apiKey: this.apiKey,
      temperature: this.temperature,
      modelName: this.model,
      configuration: {
        baseURL: this.baseUrl,
        defaultHeaders: {
          "X-Accel-Buffering": "no",
          "Cache-Control": "no-cache",
        },
        fetch: tauriFetch,
      },
      topP: this.topP,
      maxTokens: this.maxToken,
      streaming: true,
    });
    return llm;
  }
  async save() {
    try {
      // save to password manager
      if (this.apiKey) {
        PassWordManager.savePassword(LLMConfig.key, this.apiKey, true);
      } else {
        try {
          PassWordManager.deletePassword(LLMConfig.key);
        } catch (e) {
          console.log("delete passwd failed", e);
        }
      }
      this.apiKey = "";
      appStore.set(LLMConfig.key, this);
    } catch (e) {
      console.error("save llm config failed", e);
      message("save llm config failed", { kind: "error" });
    }
  }

  enable() {
    return this.baseUrl && this.model;
  }

  static async get(): Promise<LLMConfig | null> {
    let ret = await appStore.get<LLMConfig | null>(LLMConfig.key);
    if (ret) {
      ret = { ...new LLMConfig(), ...ret } as LLMConfig;
      Object.setPrototypeOf(ret, LLMConfig.prototype);
      if (ret.enable()) {
        const pass = await PassWordManager.getPassword(LLMConfig.key);
        ret.apiKey = pass;
      }
    }
    return ret;
  }
}
enum MongoLanguage {
  js = "javascript",
  ts = "typescript",
}
const resizeHandlerSize = 4;

// this must be the same as the rust Result
interface QueryRespData {
  Ok?: MuiDataGridData;
  Err?: string;
}

function newEmptyResp(): MuiDataGridData {
  return {
    fields: [],
    data: [],
    sql: "",
    total: 0,
    cache_time: 0,
    has_more: false,
  };
}

const MaxTreeContext = createContext<number>(120);

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
  errToString,
  getViewportSize,
  isTypeLargeText,
  DataSourceKind,
  connTypeToDataSourceKind,
  secondConfirm,
  appStore,
  ColumnWidth,
  handleCopyToClipboard,
  validJson,
  TabActiveContext,
  FavoriteSql,
  MatchPatternContext,
  quoteDatabaseIdentifier,
  saveAndSetSize,
  SaveDomSizes,
  DocsAndCount,
  UserSetting,
  SettingContext,
  SetCloseConfirmTextContext,
  WindowFocusContext,
  editorSetPositionAndReveal,
  postgresLike,
  getEditorText,
  executeSqlWithoutConn,
  QueryLogHandler,
  ColorFn,
  getSupportExportKind,
  SavePageSize,
  rustResultOk,
  generateDefaultFilename,
  pageSizeOptions,
  textEllipsisStyle,
  SaveAlign,
  SaveMatchPattern,
  SaveLeftTopShow,
  RedisTableType,
  extractFieldAndIDFromMouseEvent,
  editorPositionToEnd,
  KeyPressed,
  dataGridStyle,
  isMacOs,
  LoggerLevel,
  ViewKind,
  registeredThemes,
  getLanguageFromDataSourceKind,
  SaveQueryLogConfigure,
  UserSettingAppearance,
  UserSettingExport,
  UserSettingEditor,
  settingChangeEvent,
  UserSettingMisc,
  dataGridheaderFontSize,
  TableType,
  appTheme,
  isDarkTheme,
  appDatabase,
  shrinkAble,
  isWindows,
  LLMConfig,
  LLMConfigContext,
  MongoLanguage,
  extractLastNameFromUuid,
  resizeHandlerSize,
  prettyMs,
  newEmptyResp,
  MaxTreeContext,
};

type SetStatusFn = (s: string) => void;

export type { ErrAndLoc, QueryRespData, SetStatusFn };

async function webviewWIndowInit() {
  const currentWebview = WebviewWindow.getCurrent();
  appStore.get<number>("zoom").then(async zoom => {
    if (zoom) {
      try {
        await currentWebview.setZoom(zoom);
      } catch (e) {
        console.error("set zoom failed", e);
        message("set zoom failed : " + String(e), { kind: "error" });
      }
    }
  });
  if (["windows", "linux"].includes(platform())) {
    document.addEventListener("keydown", function (event) {
      if (event.key === "F11") {
        currentWebview.isFullscreen().then(isFull => {
          currentWebview.setFullscreen(!isFull);
        });
      }
    });
  }
  currentWebview.theme().then(async theme => {
    if (!theme) {
      return;
    }
    appTheme = theme;
    notifySettingChange();
  });
  currentWebview.onThemeChanged(async msg => {
    appTheme = msg.payload;
    notifySettingChange();
  });
  if (platform() == "linux") {
    //  https://github.com/tauri-apps/tauri/issues/9427
    setInterval(() => {
      currentWebview.theme().then(theme => {
        if (appTheme == theme) {
          return;
        }
        appTheme = theme;
        notifySettingChange();
      });
    }, 3000);
  }

  if (!import.meta.env.DEV) {
    document.addEventListener("contextmenu", function (event) {
      event.preventDefault();
    });

    document.addEventListener("keydown", function (event) {
      // Prevent F5 or Ctrl+R (Windows/Linux) and Command+R (Mac) from refreshing the page
      if (
        event.key === "F5" ||
        (event.ctrlKey && event.key === "r") ||
        (event.metaKey && event.key === "r")
      ) {
        event.preventDefault();
      }
    });
  }
}
webviewWIndowInit();
