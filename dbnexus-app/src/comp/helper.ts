import { loader } from "@monaco-editor/react";

import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

self.MonacoEnvironment = {
  getWorker(_, label) {
    if (label === "json") {
      return new jsonWorker();
    }
    if (label === "css" || label === "scss" || label === "less") {
      return new cssWorker();
    }
    if (label === "html" || label === "handlebars" || label === "razor") {
      return new htmlWorker();
    }
    if (label === "typescript" || label === "javascript") {
      return new tsWorker();
    }
    return new editorWorker();
  },
};

loader.config({ monaco });

import { editor, languages, Position } from "monaco-editor";
import types from "./ApiStruct";
import {
  createContext,
  CSSProperties,
  MouseEvent,
  ReactElement,
  ReactNode,
  RefObject,
} from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { Store } from "@tauri-apps/plugin-store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Channel, invoke } from "@tauri-apps/api/core";
import { BSON } from "bson";
import Database, { QueryResult } from "@tauri-apps/plugin-sql";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import themelist from "../../monaco-themes/themes/themelist.json";

import { PassWordManager } from "./Common";

import prettyMilliseconds from "pretty-ms";
import { emitTo } from "@tauri-apps/api/event";
import { cancelTask } from "./handlers";

const pageSizeOptions = [10, 20, 50, 100];
const appStore = await Store.load("store.json");
function prettyMs(x: number) {
  return prettyMilliseconds(x);
}

let appDatabase: Database = await Database.load("sqlite:dbpal.sqlite.db");

appDatabase.select("select * from column_width").then((xs) => {
  (xs as { name: string; width: number }[]).forEach((e) => {
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
      .catch((e) => {
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
    } order by ${
      order_by ? order_by : "create_at desc"
    } limit ${pageSize} offset ${pageSize * (page - 1)}`;
    console.info("query log sql:", sql);
    return appDatabase.select(sql, [this.uuid]);
  }
  async queryCount(matchSql: string): Promise<number> {
    const sql = `select count(*) as total from query_log where uuid=? ${
      matchSql ? `and sql like '%${matchSql}%'` : ""
    }`;
    return appDatabase.select(sql, [this.uuid]).then((
      e,
    ) => (e ? e[0].total : 0));
  }
  async delete(): Promise<QueryResult> {
    return appDatabase.execute("delete from query_log where uuid=?", [
      this.uuid,
    ]);
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
  const characters =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
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
    return new TextDecoder("utf-8", { fatal: true }).decode(
      new Uint8Array(bytes),
    );
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
    .map((byte) => byte.toString(16).padStart(2, "0")) // 转换每个字节为十六进制并补齐
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
  CouchBase = "couchbase",
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
    toUpper.includes("TEXT") || toUpper.includes("LONG") ||
    toUpper.includes("CLOB")
  );
}

class SaveBoolToLocalStorage {
  constructor(public key: string) {}
  get() {
    return localStorage.getItem(this.key) == "1";
  }
  save(x: boolean) {
    localStorage.setItem(this.key, x ? "1" : "0");
  }
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
      .then((xs) => {
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
      .then((x) => x.map((y) => y.uuid));
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
      appDatabase.execute(`delete from favorite_sql where connection_name=?`, [
        name,
      ]);
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
    return appDatabase.execute("delete from column_width").then((x) => {
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
    .catch((e) => {
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
  setSelectTab;
  appendTab(tab: TabContent);
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
  const e = new ResizeObserver((en) => {
    const height = en[0].target.clientHeight;
    const width = en[0].target.clientWidth;
    if (width && height) {
      SaveDomSizes.saveSize(key, { width, height });
    }
  });
  SaveDomSizes.getSize(key).then((size) => {
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
  constructor(
    public docs: BSON.Document[],
    public count: number,
  ) {}
}

function notifySettingChange() {
  // due to this issue https://github.com/tauri-apps/tauri/issues/14750
  setTimeout(() => {
    emitTo(
      { label: WebviewWindow.getCurrent().label, kind: "WebviewWindow" },
      settingChangeEvent,
    );
  }, Math.random() * 1000);
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
  async save() {
    await appStore.set(UserSettingAppearance.key, this);
    notifySettingChange();
  }
  // 可能和以前的设置冲突
  datagridRowHeight() {
    return Math.ceil(this.fontSize * this.datagridRowHeightFactor);
  }
  static async get(): Promise<UserSettingAppearance> {
    try {
      let setting = await appStore.get<UserSettingAppearance>(
        UserSettingAppearance.key,
      );
      setting = {
        ...new UserSettingAppearance(),
        ...setting,
      } as UserSettingAppearance;
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
  async save() {
    await appStore.set(UserSettingExport.key, this);
    notifySettingChange();
  }
  static async get(): Promise<UserSettingExport> {
    try {
      let setting = await appStore.get<UserSettingExport>(
        UserSettingExport.key,
      );
      setting = { ...new UserSettingExport(), ...setting } as UserSettingExport;
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
  async save() {
    await appStore.set(UserSettingEditor.key, this);
    notifySettingChange();
  }

  static async get(): Promise<UserSettingEditor> {
    try {
      const x = await appStore.get<UserSettingEditor>(UserSettingEditor.key);
      return { ...new UserSettingEditor(), ...x } as UserSettingEditor;
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingEditor();
    }
  }
}

class UserSetting {
  editor: UserSettingEditor;
  appearance: UserSettingAppearance;
  export: UserSettingExport;
  misc: UserSettingMisc;
  editorTheme(): string {
    if (this.editor.theme) {
      return this.editor.theme;
    }
    return isDarkTheme() ? "vs-dark" : "light";
  }

  static async load() {
    try {
      const ret = new UserSetting();
      ret.editor = await UserSettingEditor.get();
      ret.appearance = await UserSettingAppearance.get();
      ret.misc = await UserSettingMisc.get();
      ret.export = await UserSettingExport.get();
      return ret;
    } catch (e) {
      console.error("load config failed,err:", e);
      message("could load config" + String(e), { kind: "error" });
    }
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
  async save() {
    await appStore.set(UserSettingMisc.key, this);
    notifySettingChange();
  }

  static async get(): Promise<UserSettingMisc> {
    try {
      const x = await appStore.get<UserSettingMisc>(UserSettingMisc.key);
      return { ...new UserSettingMisc(), ...x } as UserSettingMisc;
    } catch (e) {
      console.log("get user setting failed, use default", e);
      return new UserSettingMisc();
    }
  }
}

const SettingContext = createContext<UserSetting>(null);

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
  appStore.get<string>(LoggerLevel.key).then((level) => {
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

function executeSqlWithoutConn(uuid: string, sql: string) {
  return invoke<QueryRespData>("simple_execute_sql", {
    name: uuid,
    sql,
  });
}

class ColorFn {
  static hexToRgb(hex) {
    hex = hex.replace("#", "");
    if (hex.length === 3) hex = [...hex].map((c) => c + c).join("");
    const num = parseInt(hex, 16);
    return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
  }
  static isLightHex(hex) {
    const [r, g, b] = ColorFn.hexToRgb(hex);
    const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
    return lum > 0.5;
  }
  static isColor = (str) => {
    const o = new Option().style;
    o.color = str;
    return o.color !== "";
  };
}

class SavePageSize {
  key: string;

  defaultValue: number;

  constructor(dv: number, k = "") {
    this.defaultValue = dv;

    this.key = "dbpal-page-size" + "-" + k;
  }

  save(size: number) {
    localStorage.setItem(this.key, size + "");
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
    return this.defaultValue;
  }
  static instance = new SavePageSize(100);
  static couchbase = new SavePageSize(10, "couchbase");
  static mongodb = new SavePageSize(10, "mongodb");
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

  async invoke(msg: string): Promise<string> {
    return await invoke<string>("openai_invoke", {
      req: this.makeReq(msg),
    });
  }

  makeReq(msg: string): types.OpenAIReq {
    return {
      url: this.baseUrl,
      api_key: this.apiKey,
      model: this.model,
      msg,
      temperature: this.temperature,
      max_tokens: this.maxToken,
      top_p: this.topP,
    };
  }
  async stream(
    msg: string,
    handleMessage: (_: string) => void,
    handleCancel: (_: () => void) => void,
  ) {
    const taskId = generateRandomString();
    const emit = new Channel<string>((event) => {
      handleMessage(event);
    });
    handleCancel(() => {
      cancelTask(taskId);
    });
    await invoke("openai_stream", { req: this.makeReq(msg), emit, taskId });
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
  };
}

const MaxTreeContext = createContext<number>(120);

const ChatShowContext = createContext<{
  show: boolean;
  setShow: (_: boolean) => void;
  doQuery: RefObject<() => void>;
}>(null);
const saveQueryLog = new SaveBoolToLocalStorage("save-query-log");

export {
  appDatabase,
  appStore,
  appTheme,
  backendCompletionItemToEditor,
  ChatShowContext,
  ColorFn,
  ColumnWidth,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  dataGridheaderFontSize,
  dataGridStyle,
  DataSourceKind,
  DocsAndCount,
  editorPositionToEnd,
  editorSetPositionAndReveal,
  errToString,
  executeSqlWithoutConn,
  extractConfigNameFromUuid,
  extractFieldAndIDFromMouseEvent,
  extractLastNameFromUuid,
  FavoriteSql,
  generateDefaultFilename,
  generateRandomString,
  getEditorText,
  getLanguageFromDataSourceKind,
  getSupportExportKind,
  getViewportSize,
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  isDarkTheme,
  isMacOs,
  isTypeLargeText,
  isWindows,
  KeyPressed,
  LLMConfig,
  LLMConfigContext,
  loader,
  LoggerLevel,
  MatchPatternContext,
  MaxTreeContext,
  MongoLanguage,
  newEmptyResp,
  pageSizeOptions,
  postgresLike,
  prettyMs,
  QueryLogHandler,
  quoteDatabaseIdentifier,
  RedisTableType,
  registeredThemes,
  resizeHandlerSize,
  rustResultOk,
  SaveAlign,
  saveAndSetSize,
  SaveBoolToLocalStorage,
  SaveDomSizes,
  SaveMatchPattern,
  SavePageSize,
  saveQueryLog,
  secondConfirm,
  SetCloseConfirmTextContext,
  settingChangeEvent,
  SettingContext,
  shrinkAble,
  stringToHex,
  TabActiveContext,
  TabContent,
  TableType,
  TabsSetContext,
  textEllipsisStyle,
  UserSetting,
  UserSettingAppearance,
  UserSettingEditor,
  UserSettingExport,
  UserSettingMisc,
  UuidContext,
  validJson,
  ViewKind,
};

export type { ErrAndLoc, QueryRespData };

async function webviewWIndowInit() {
  const currentWebview = WebviewWindow.getCurrent();
  appStore.get<number>("zoom").then(async (zoom) => {
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
        currentWebview.isFullscreen().then((isFull) => {
          currentWebview.setFullscreen(!isFull);
        });
      }
    });
  }
  currentWebview.theme().then(async (theme) => {
    if (!theme) {
      return;
    }
    appTheme = theme;
    notifySettingChange();
  });
  currentWebview.onThemeChanged(async (msg) => {
    appTheme = msg.payload;
    notifySettingChange();
  });
  if (platform() == "linux") {
    // https://github.com/tauri-apps/tauri/issues/9427
    setInterval(() => {
      currentWebview.theme().then((theme) => {
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
