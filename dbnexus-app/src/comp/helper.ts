import { MonacoEditor } from "./Editor";
import * as monaco from "monaco-editor";
import { editor, languages, Position } from "monaco-editor";
import {
  createContext,
  CSSProperties,
  MouseEvent,
  ReactElement,
  ReactNode,
} from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { BSON } from "bson";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import themelist from "../../monaco-themes/themes/themelist.json";
import prettyMilliseconds from "pretty-ms";
import { v4 as uuidv4 } from "uuid";
import {
  commands,
  CompletionItem,
  CompletionKind,
  ErrAndLoc,
  MuiDataGridData,
} from "./bindings";
import { NotificationParams } from "./store";

const pageSizeOptions = [10, 20, 50, 100];
function prettyMs(x: number) {
  return prettyMilliseconds(x);
}

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

// redis的数据存储的时候都是二进制 有时候包含的是有效的
// utf8字符前台的处理的最好当成utf8来处理
// 不然当成二进制数据来处理呢
// 后台放回数据的时候都是 hex string模式

function hexTryToString(hex: string): string | null {
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
    .map((byte) => byte.toString(16).padStart(2, "0")) // 转换每个字节为十六进制并补齐
    .join(""); // 连接为一个字符串
}

function stringToHex(str: string): string {
  return arrayToHex(stringToHexArray(str));
}

function backendCompletionItemToEditor(xs: CompletionItem[]): languages.CompletionList {
  function getKind(kind: CompletionKind): languages.CompletionItemKind {
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

function errToString(message: any) {
  if (typeof message == "object" && "reason" in message) {
    message = message["reason"];
  }
  if (typeof message == "object" && message instanceof Error) {
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

function quoteDatabaseIdentifier(identifier: string, kind: DataSourceKind): string {
  let q = "";
  switch (kind) {
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

const TabActiveContext = createContext(true);

const handleCopyToClipboard = (
  text,
  showNotification?: (_: NotificationParams) => void,
  okCallBack?,
) => {
  writeText(text)
    .then(() => {
      if (showNotification) {
        showNotification({ severity: "success", title: "success copy to clipboard." });
      }
      if (okCallBack) {
        okCallBack();
      }
    })
    .catch((e) => {
      if (showNotification) {
        showNotification({
          severity: "warning",
          title: "copy to clipboard failed",
          message: e,
        });
      }
    });
};

class TabContent {
  constructor(
    public header: string,
    public content: ReactNode,
    public icon: ReactElement,
    public id: string = uuidv4(),
  ) {}
}

const TabContext = createContext<{ id: string }>(null);

const UuidContext = createContext<{
  uuid: string;
  kind: DataSourceKind;
}>(null);

function validJson(s: string): Promise<ErrAndLoc | null> {
  return commands.validJson(s).then((data) => data);
}

class DocsAndCount {
  constructor(
    public docs: BSON.Document[],
    public count: number,
  ) {}
}

function editorSetPositionAndReveal(
  editor: editor.IStandaloneCodeEditor,
  pos: Position,
) {
  editor.setPosition(pos);
  editor.focus();
  editor.revealLines(pos.lineNumber, pos.lineNumber);
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
    showNotification({ severity: "warning", title: "empty statement" });
    return undefined;
  }
  return text;
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

function getSupportExportKind(kind: DataSourceKind, isTable: boolean) {
  switch (kind) {
    case DataSourceKind.Mongo:
    case DataSourceKind.CouchBase:
      return ["json", "jsonline"];
    default:
      return isTable
        ? ["csv", "sql", "json", "jsonline", "xlsx", "html", "markdown"]
        : ["csv", "json", "jsonline", "xlsx", "html", "markdown"];
  }
}

function generateDefaultFilename(name: string, extension: string) {
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
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
  flexShrink: 1,
  minWidth: "1rem",
} as CSSProperties;

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

  static editorMarkdown(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "b");
  }

  static prevPage(e: KeyboardEvent) {
    return (
      (isMacOs() && e.metaKey && e.key === "ArrowUp") ||
      (!isMacOs() && e.ctrlKey && e.key === "ArrowUp")
    );
  }

  static nextPage(e: KeyboardEvent) {
    return (
      (isMacOs() && e.metaKey && e.key === "ArrowDown") ||
      (!isMacOs() && e.ctrlKey && e.key === "ArrowDown")
    );
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
    const themes = await commands.monacoEditorThemes();
    const themeObject = {};
    for (const key in themes) {
      themeObject[key] = JSON.parse(themes[key]);
    }
    for (const key in themelist) {
      const v = themelist[key];
      monaco.editor.defineTheme(key, themeObject[v]);
      registeredThemes.push(key);
    }
  } catch (e) {
    message("set up editor themes failed,err:" + String(e), { kind: "error" });
    console.error("set up editor themes failed", e);
  }
}

setUpEditorThemes();

const dataGridheaderFontSize = "0.8rem";

const shrinkAble = {
  flexShrink: 1,
  overflow: "hidden",
  minWidth: "3rem",
} as CSSProperties;

enum MongoLanguage {
  js = "javascript",
  ts = "typescript",
}
const resizeHandlerSize = 4;

// this must be the same as the rust Result
export interface QueryRespData {
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

const ChatShowContext = createContext<{
  show: boolean;
  setShow: (_: boolean) => void;
}>(null);

const saveQueryKey = "query-4k9zF7m5p3T1H2";

function isDarkStr(x: string) {
  return x == "dark";
}

const QueryIDEditorChatCtx = createContext<{ queryId: string; editor; setEditor }>(
  null,
);

export type AppCodeExtraButtons = (_: { code: string; language: string }) => ReactNode;
export type AppCodeHanleEdit = (_: { code: string }) => void;

/**
 * Focuses and scrolls to the first occurrence of a text string in the editor.
 * @param editor The monaco editor instance
 * @param searchText The string to search for
 * @param caseSensitive Whether to match case (default: false)
 */
export function jumpToEditorText(
  editor: monaco.editor.IStandaloneCodeEditor,
  searchText: string,
  caseSensitive: boolean = false,
) {
  const model = editor.getModel();
  if (!model || !searchText) return;

  // findMatches(searchString, searchOnlyEditableRange, isRegex, matchCase, wordSeparators, captureMatches)
  const matches = model.findMatches(
    searchText,
    false, // searchOnlyEditableRange
    false, // isRegex
    caseSensitive,
    null, // wordSeparators
    false, // captureMatches
  );

  if (matches.length > 0) {
    const firstMatchRange = matches[0].range;

    // 1. Scroll the editor so the match is in the center
    editor.revealRangeInCenter(firstMatchRange, monaco.editor.ScrollType.Smooth);

    // 2. Set the cursor/selection to that range
    editor.setSelection(firstMatchRange);

    // 3. Bring focus to the editor
    editor.focus();

    console.log(`Found ${matches.length} matches. Jumping to the first one.`);
  } else {
    console.warn("Text not found in the editor.");
  }
}

export function getDefaultLanguage(kind: DataSourceKind) {
  switch (kind) {
    case DataSourceKind.Mongo:
      return "javascript";
    case DataSourceKind.Redis:
      return "redis";
    default:
      return "sql";
  }
}
export {
  isDarkStr,
  saveQueryKey,
  backendCompletionItemToEditor,
  ChatShowContext,
  ColorFn,
  connTypeToDataSourceKind,
  dataGridheaderFontSize,
  dataGridStyle,
  DataSourceKind,
  DocsAndCount,
  editorPositionToEnd,
  editorSetPositionAndReveal,
  errToString,
  extractConfigNameFromUuid,
  extractFieldAndIDFromMouseEvent,
  generateDefaultFilename,
  getEditorText,
  getSupportExportKind,
  handleCopyToClipboard,
  hexToString,
  hexTryToString,
  isMacOs,
  isTypeLargeText,
  isWindows,
  KeyPressed,
  MongoLanguage,
  newEmptyResp,
  pageSizeOptions,
  prettyMs,
  quoteDatabaseIdentifier,
  RedisTableType,
  registeredThemes,
  resizeHandlerSize,
  rustResultOk,
  secondConfirm,
  TabContext,
  shrinkAble,
  stringToHex,
  TabActiveContext,
  TabContent,
  textEllipsisStyle,
  UuidContext,
  validJson,
  ViewKind,
  MonacoEditor,
  QueryIDEditorChatCtx,
};

async function webviewWIndowInit() {
  const currentWebview = WebviewWindow.getCurrent();
  if (["windows", "linux"].includes(platform())) {
    document.addEventListener("keydown", function (event) {
      if (event.key === "F11") {
        currentWebview.isFullscreen().then((isFull) => {
          currentWebview.setFullscreen(!isFull);
        });
      }
    });
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
