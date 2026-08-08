import { MonacoEditor } from "./Editor";
import * as monaco from "monaco-editor";
import { editor, languages, Position } from "monaco-editor";
import {
  createContext,
  CSSProperties,
  Dispatch,
  MouseEvent,
  ReactElement,
  ReactNode,
  RefObject,
  SetStateAction,
} from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { BSON } from "bson";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import prettyMilliseconds from "pretty-ms";
import { CompletionItem, CompletionKind, Field, MuiDataGridData } from "./bindings";
import { NotificationParams } from "./store";
import { extractBaseName, uuidv4 } from "./Common";
import { GridApi } from "@mui/x-data-grid";

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

function connTypeToDataSourceKind(x: string): DataSourceKind {
  return x as DataSourceKind;
}
function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  return (
    toUpper.includes("TEXT") || toUpper.includes("LONG") || toUpper.includes("CLOB")
  );
}

const TabActiveContext = createContext(true);

const handleCopyToClipboard = async (
  text: string,
  showNotification: (_: NotificationParams) => void,
  okCallBack?: () => void,
) => {
  try {
    if ((await readText()) == text) {
      showNotification({ severity: "info", title: "Already copied to clipboard." });
      return;
    }
  } catch (e) {
    console.error("read clipboard failed", e);
  }

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

function getEditorText(
  editor: editor.IStandaloneCodeEditor,
  showNotification: (_: NotificationParams) => void,
  trim: boolean,
) {
  const range = editor.getSelection();
  let text: string;
  if (range) {
    text = editor.getModel().getValueInRange(range);
  }
  if (!text) {
    text = editor.getValue();
  }
  if (trim) {
    text = text.trim();
  }
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
  let r = [];
  switch (kind) {
    case DataSourceKind.Mongo:
    case DataSourceKind.CouchBase:
      r = ["json", "jsonline"];
      break;
    default:
      r = isTable
        ? ["csv", "sql", "json", "jsonline", "xlsx", "html", "markdown"]
        : ["csv", "json", "jsonline", "xlsx", "html", "markdown"];
      break;
  }
  if (-1 != r.indexOf("csv")) {
    r = ["tsv", ...r];
  }
  return r;
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
const textEllipsisStyle: CSSProperties = {
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
  flexShrink: 1,
  minWidth: "1rem",
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

  static editorMarkdown(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "b");
  }

  static prevPage(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "ArrowUp");
  }

  static nextPage(e: KeyboardEvent) {
    return KeyPressed.matchControl(e, "ArrowDown");
  }
}

function isMacOs() {
  return platform() == "macos";
}

function isWindows() {
  return platform() == "windows";
}

const dataGridStyle: CSSProperties = {
  flexGrow: 1,
  height: "100px",
  flexShrink: 1,
  // 这个值用 0% 在windows也有问题好像会把datagrid变得很高 用0 都可以 用0% 都不可以 然而flex为1 设置的值是0% 可能是兼容性的问题
  flexBasis: "0",
};

enum ViewKind {
  View,
  MaterializedView,
}

const registeredThemes = [];

async function setUpEditorThemes() {
  try {
    const modules = import.meta.glob("../../monaco-themes/themes/*.json", {
      eager: true,
    });

    Object.entries(modules).forEach(([path, module]) => {
      // 1. 提取文件名并清理非法字符（去掉空格）
      let themeName = extractBaseName(path).slice(0, -5);

      if (themeName == "themelist") {
        return;
      }
      themeName = themeName
        .replaceAll(" ", "-")
        .replaceAll("(", "")
        .replaceAll(")", "");

      // 2. 确保传入的是对象内容，而不是整个 module
      const themeData = (module as any).default;
      try {
        monaco.editor.defineTheme(themeName, themeData);
        registeredThemes.push(themeName);
        // if (import.meta.env.DEV) console.log(`define theme successfully: ${themeName}`);
      } catch (e) {
        if (import.meta.env.DEV) {
          console.error(`define theme ${themeName} failed:`, e);
        }
      }
    });
  } catch (e) {
    message("set up editor themes failed,err:" + String(e), { kind: "error" });
    console.error("set up editor themes failed", e);
  }
}

setUpEditorThemes();

const dataGridheaderFontSize = "0.8rem";

const shrinkAble: CSSProperties = {
  flexShrink: 1,
  overflow: "hidden",
  minWidth: "3rem",
};

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

const QueryIDEditorChatCtx = createContext<{
  queryId: string;
  editor: editor.IStandaloneCodeEditor;
  setEditor: Dispatch<SetStateAction<editor.IStandaloneCodeEditor>>;
}>(null);

export type AppCodeExtraUI = (_: { code: string; language: string }) => ReactNode;
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
      return "typescript";
    case DataSourceKind.Redis:
      return "redis";
    default:
      return "sql";
  }
}

function clickDataGridColumn(
  api: RefObject<GridApi>,
  event: MouseEvent,
  field: string,
  set: Dispatch<SetStateAction<Set<string>>>,
) {
  const isControl = isMacOs() ? event.metaKey : event.ctrlKey;

  if (!isControl) {
    allGridHeaderTitle(api, (dom) => {
      console.log("xxxx", dom);
      dom.classList.remove(gridColumnHeaderSelectClassName);
    });
  }

  api.current
    .getColumnHeaderElement(field)
    .children[0].children[0].children[0].classList.add(gridColumnHeaderSelectClassName);

  set((old) => {
    if (isControl) {
      const c = new Set(old);
      c.add(field);
      return c;
    } else {
      return new Set([field]);
    }
  });
}

class DatabaseOperation {
  public constructor(
    public op: DatabaseOpKind,
    public sql: string,
    public tableId: string,
    public newValue: object | null,
  ) {}
}

export enum DatabaseOpKind {
  Update,
  Delete,
}

enum AppColumnMenuKind {
  SqlTable,
  QueryResp,
  Mongo,
  Redis,
}

const SqlOpContext = createContext<{
  ops: DatabaseOperation[];
  setOps: Dispatch<SetStateAction<DatabaseOperation[]>>;
  fields: Field[];
  uuid: string;
  handleReload: () => void;
}>(null);

const AppColumnMenuCtx = createContext<{
  uuid?: string;
  selectColumns: string[];
  kind: AppColumnMenuKind;
}>(null);

export const gridColumnHeaderSelectClassName = "grid-cell-select-highlight";

// 有些地方不好用状态变量来做
// 这里的是命令式的写法 目前就是
// 用来管理一些css呢
function allGridHeaderTitle(
  api: RefObject<GridApi>,
  callback: (_: HTMLDivElement) => void,
) {
  api.current.getAllColumns().forEach((x) => {
    let title = api.current.getColumnHeaderElement(x.field).children[0].children[0]
      .children[0] as HTMLDivElement;
    if (title) {
      callback(title);
    } else {
      console.error("should beable to find title,but null");
    }
  });
}

function escapeTSV(x: string): string {
  if (!x) {
    return;
  }
  // If the string contains quotes, tabs, or newlines, it must be escaped and wrapped
  if (x.includes('"') || x.includes("\t") || x.includes("\n") || x.includes("\r")) {
    return `"${x.replaceAll('"', '""')}"`;
  }
  return x;
}

function parseCellType(cell: string): string | number | null {
  if (!cell) {
    return null;
  }
  // 1. 去除首尾空格
  let value = cell.trim();

  // 2. 如果是空字段，直接转为 null
  if (value === "") {
    return null;
  }

  // 3. 处理 TSV 的双引号包裹和内部双引号转义
  // 例如: "Jane""" -> Jane"  /  "35" -> 35
  if (value.startsWith('"') && value.endsWith('"')) {
    // 移除最外层的双引号，并将内部的 "" 还原为 "
    value = value.slice(1, -1).replaceAll('""', '"');

    // 如果剥离外层引号后变为空，说明原本就是个空值
    if (value === "") return null;
  }

  // 4. 尝试转换为数字 (Int / Float)
  // 排除掉不合法的数字格式（如空字符、多余的符号等）
  const num = Number(value);
  if (!isNaN(num) && value !== "") {
    return num;
  }

  // 5. 其余情况一律作为原始字符串返回（包括 "true", "false", "00123" 等）
  // 注意：如果你不想让类似 "00123" 这种带前导零的工号/编码变成数字，
  // 可以加上判断：if (!isNaN(num) && !value.startsWith('0')) { return num; }
  return value;
}

function arrayToMarkdownTable(data: string[][]): string {
  if (!data || data.length === 0) return "";

  // 1. 数据清洗辅助函数：处理 TSV 外层引号、转义引号，并安全处理换行符
  const cleanCell = (cell: string): string => {
    let val = (cell || "").trim();

    // 脱掉 TSV 的外层双引号，还原内部的 "" 为 "
    if (val.startsWith('"') && val.endsWith('"')) {
      val = val.slice(1, -1).replaceAll('""', '"');
    }

    // Markdown 表格单行内不能有换行符，否则会错位。将换行转为 HTML 换行
    return val.replace(/\r?\n/g, "<br>");
  };

  // 2. 提取并处理第一行作为表头 (Title)
  const headers = data[0].map(cleanCell);

  // 3. 生成 Markdown 的对齐分隔线 (---|---|---)
  const separator = headers.map(() => "---");

  // 4. 处理剩余的数据行
  const rows = data.slice(1).map((row) => row.map(cleanCell));

  // 5. 组装成标准的 Markdown 格式
  const markdownLines = [
    `| ${headers.join(" | ")} |`,
    `| ${separator.join(" | ")} |`,
    ...rows.map((row) => `| ${row.join(" | ")} |`),
  ];

  return markdownLines.join("\n");
}

interface EscapeOptions {
  /** Use strict escaping for platforms like Telegram MarkdownV2 */
  strict?: boolean;
}

/**
 * Escapes special characters to prevent Markdown formatting.
 * @param text The raw input string to escape.
 * @param options Configuration for escaping behavior.
 * @returns The escaped safe string.
 */
function escapeMarkdown(text: string, options: EscapeOptions = {}): string {
  if (!text) return "";

  // Standard Markdown characters
  // eslint-disable-next-line no-useless-escape
  let pattern = /([\\*_`\[\](){}#\+\-\.!\|>])/g;

  // Strict mode: includes extra characters like symbols, equals, and tilde (~=+=^|等)
  if (options.strict) {
    // eslint-disable-next-line no-useless-escape
    pattern = /([\\*_`\[\](){}#\+\-\.!\|>#=~^])/g;
  }

  return text.replace(pattern, "\\$1");
}

const columnHeaderStyle: CSSProperties = {
  minWidth: "5rem",
  border: "2px dashed gray",
  borderRadius: "4px",
  padding: "4px",
  cursor: "default",
};

/**
 * Converts a string array into a standard CSV format (one column).
 * @param data - Array of strings or primitive values to format.
 * @returns The formatted CSV string.
 */
function createCSV(data: string[]): string {
  if (!data || data.length === 0) {
    return "";
  }

  return (
    data
      .map((item) => {
        // Convert to string (handles numbers, booleans, etc.)
        let cell = String(item ?? "");

        // RFC 4180: If the field contains a comma, quote, or newline,
        // it must be enclosed in double quotes
        const needsQuotes = /[",\n\r]/.test(cell);

        if (needsQuotes) {
          // Escape existing double quotes by doubling them
          cell = cell.replace(/"/g, '""');
          cell = `"${cell}"`;
        }

        return cell;
      })
      .join(",") + "\n"
  );
}

/**
 * 判断文件后缀名是否为Excel格式
 * @param fileName - 文件名或文件路径
 * @returns 是否为Excel文件
 */
function isExcelFile(fileName: string): boolean {
  // 定义所有Excel支持的后缀名
  const excelExtensions = [
    ".xls", // Excel 97-2003
    ".xlsx", // Excel 2007+
    ".xlsm", // Excel 启用宏的工作簿
    ".xlsb", // Excel 二进制工作簿
    ".xlam", // Excel 加载项
    ".xltx", // Excel 模板
    ".xltm", // Excel 启用宏的模板
  ];

  // 获取文件后缀名（转为小写以支持大小写不敏感）
  const extension = fileName.substring(fileName.lastIndexOf(".")).toLowerCase();

  // 判断后缀名是否在列表中
  return excelExtensions.includes(extension);
}

export {
  isExcelFile,
  escapeMarkdown,
  createCSV,
  AppColumnMenuKind,
  SqlOpContext,
  columnHeaderStyle,
  parseCellType,
  arrayToMarkdownTable,
  escapeTSV,
  allGridHeaderTitle,
  clickDataGridColumn,
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
  ViewKind,
  MonacoEditor,
  QueryIDEditorChatCtx,
  AppColumnMenuCtx,
  DatabaseOperation,
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
