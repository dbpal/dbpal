import { SxProps } from "@mui/material";
import { editor, languages, Position } from "monaco-editor";
import { format as formatSql, SqlLanguage } from "sql-formatter";
import { Channel } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import * as keyring from "tauri-plugin-keyring-api";

import { v4 as uuidv4 } from "uuid";
import {
  backendCompletionItemToEditor,
  DataSourceKind,
  QueryRespData,
  secondConfirm,
} from "./helper.ts";
import * as monaco from "monaco-editor";
import { readFile, stat } from "@tauri-apps/plugin-fs";
import { filesize } from "filesize";
import { commands, ExportKind, Field, QueryEvent, redisCommands } from "./bindings.ts";
import { NotificationParams } from "./store.tsx";
import { GridColDef } from "./datagridExport.ts";

/**
 * Converts a local file path to a Data URL (base64)
 * @param filePath - Absolute path to the image (e.g., from dialog.open)
 * @returns Promise<string> - The formatted data:image/...;base64 string
 */
const convertPathToDataUrl = async (filePath: string): Promise<string> => {
  try {
    const fileInfo = await stat(filePath);

    if (fileInfo.size > 10 * 1024 * 1024) {
      message(`file is too large ${filesize(fileInfo.size)} exceed 10M`);
      return;
    }

    // 1. Read file as Uint8Array
    const contents: Uint8Array = await readFile(filePath);

    // 2. Map extension to MIME type
    const extension = filePath.split(".").pop()?.toLowerCase() || "";
    let mimeType: string;

    switch (extension) {
      case "jpg":
      case "jpeg":
        mimeType = "image/jpeg";
        break;
      case "png":
        mimeType = "image/png";
        break;
      case "gif":
        mimeType = "image/gif";
        break;
      case "webp":
        mimeType = "image/webp";
        break;
      case "svg":
        mimeType = "image/svg+xml";
        break;
      default:
        mimeType = "image/png"; // Default fallback
    }

    // 3. Efficient Base64 conversion (Handles large files better than .reduce)
    const base64 = uint8ArrayToBase64(contents);

    return `data:${mimeType};base64,${base64}`;
  } catch (error) {
    console.error("Tauri FS Error:", error);
    throw new Error(`Failed to convert image: ${error}`);
  }
};

/**
 * Helper to convert Uint8Array to Base64 string efficiently
 */
function uint8ArrayToBase64(buffer: Uint8Array): string {
  let binary = "";
  const len = buffer.byteLength;
  for (let i = 0; i < len; i++) {
    binary += String.fromCharCode(buffer[i]);
  }
  return window.btoa(binary);
}

const modalStyle: SxProps = {
  position: "fixed",
  transform: "translate(50%, 50%)",
  bottom: "50%",
  right: "50%",
  bgcolor: "background.paper",
  border: "2px solid skyblue",
  boxShadow: 24,
  borderRadius: "0.4rem",
  p: 1,
  maxHeight: "85vh",
  minHeight: "10vh",
  minWidth: "20vw",
  maxWidth: "80vw",
  display: "flex",
  overflow: "auto",
  flexDirection: "column",
};

async function dropDatabase(props: {
  uuid: string;
  database: string;
  showNotification: (params: NotificationParams) => void;
  okCallback?: () => void;
}): Promise<void> {
  const { uuid, database, showNotification, okCallback } = props;

  const confirm = await secondConfirm({
    title: `drop database '${database}'`,
    confirmText: "Drop",
  });
  if (!confirm) {
    return;
  }
  commands
    .dropDatabase(uuid, database)
    .then(() => {
      showNotification({ severity: "success", title: "drop ok" });
      if (okCallback) {
        okCallback();
      }
    })
    .catch((e) => {
      showNotification({ severity: "error", title: "drop failed", message: e });
    });
}

async function dropSchema({
  uuid,
  schema,
  showNotification,
  okCallback,
}: {
  uuid: string;
  schema: string;
  showNotification: (params: NotificationParams) => void;
  okCallback: () => void;
}): Promise<void> {
  const confirm = await secondConfirm({
    title: `drop '${schema}'`,
    confirmText: "Drop",
  });
  if (!confirm) {
    return;
  }
  commands
    .dropSchema(uuid, schema)
    .then(() => {
      showNotification({ severity: "success", title: "Drop OK" });
      okCallback();
    })
    .catch((e) => {
      showNotification({ severity: "error", title: "Drop failure", message: e });
    });
}

function reloadOK(showNotification: (params: NotificationParams) => void): void {
  showNotification({ severity: "success", title: "data loaded." });
}

declare module "monaco-editor" {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace editor {
    interface ITextModel {
      _dbpal_query_id?: NameAndQueryId;
    }
  }
}

const autoCompletion = async (
  model: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList | undefined> => {
  const sql = model.getValue();
  const name = model._dbpal_query_id;
  if (!name) {
    console.error("editor id map to backend name is nul,this is internal bug");
    return;
  }
  try {
    const result = await commands.autoCompletion(
      name.name,
      name.queryId,
      sql,
      pos.lineNumber,
      pos.column,
    );
    const suggestions = backendCompletionItemToEditor(result.items);
    suggestions.incomplete = result.incomplete;
    return suggestions;
  } catch (e) {
    console.error("completion error", e);
  }
};

const aiTextAutoCompletion = async (
  model: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList> => {
  try {
    const text = model.getValue();
    {
      // try if this is ai text completion
      const name = model._dbpal_query_id;
      if (name) {
        const result = await commands.aiTextCompletion(
          name.name,
          text,
          pos.lineNumber,
          pos.column,
        );
        const suggestions = backendCompletionItemToEditor(result.items);
        suggestions.incomplete = result.incomplete;
        return suggestions;
      }
    }
    console.info("nothing to complete");
    return { suggestions: [] };
  } catch (e) {
    console.error("AI text completion error", e);
    return { suggestions: [] };
  }
};

monaco.languages.registerCompletionItemProvider("sql", {
  triggerCharacters: ["."],
  provideCompletionItems(model, position: Position) {
    return autoCompletion(model, position);
  },
});
monaco.languages.register({ id: "text" });

monaco.languages.registerCompletionItemProvider("text", {
  triggerCharacters: ["."],
  provideCompletionItems(model, position: Position) {
    return aiTextAutoCompletion(model, position);
  },
});

monaco.languages.registerDocumentFormattingEditProvider("sql", {
  provideDocumentFormattingEdits(model, options) {
    const name = model._dbpal_query_id;
    if (!name) {
      console.error("editor id map to backend name is nul,this is internal bug");
      return;
    }
    const text = formatSqlFn(name.kind, model.getValue(), options.tabSize);
    if (!text) {
      return null;
    }
    return [{ range: model.getFullModelRange(), text }];
  },
});

monaco.languages.registerCompletionItemProvider("redis", {
  provideCompletionItems(model, position: Position) {
    return autoCompletion(model, position);
  },
});
function formatSqlFn(kind: DataSourceKind, sql: string, tabSize: number): string {
  const opts = {
    tabSize,
    insertSpaces: true,
  };
  try {
    // https://github.com/sql-formatter-org/sql-formatter/blob/master/docs/language.md
    let language: SqlLanguage = "sql";
    switch (kind) {
      case DataSourceKind.DuckDB:
        language = "duckdb";
        break;
      case DataSourceKind.Mysql:
        language = "mysql";
        break;
      case DataSourceKind.MariaDB:
        language = "mariadb";
        break;
      case DataSourceKind.TiDB:
        language = "tidb";
        break;
      case DataSourceKind.Oracle:
        language = "plsql";
        break;
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        language = "postgresql";
        break;
      case DataSourceKind.LibSql:
      case DataSourceKind.Sqlite:
        language = "sqlite";
        break;
    }
    return formatSql(sql, {
      language,
      tabWidth: opts.tabSize,
      useTabs: !opts.insertSpaces,
    });
  } catch (e) {
    message("format failed error:" + String(e), { kind: "error" });
    console.error("format sql failed error:", e);
    return sql;
  }
}

class QueryConn {
  queryId: string;
  uuid: string;
  kind: DataSourceKind;

  constructor(uuid: string, queryId: string, kind: DataSourceKind) {
    this.queryId = queryId;
    this.uuid = uuid;
    this.kind = kind;
  }
  async open(): Promise<void> {
    console.log("open connection", this.uuid, this.queryId);
    return commands.querySqlOpenConnection(this.uuid, this.queryId);
  }

  supportTransaction(): boolean {
    return [
      DataSourceKind.Oracle,
      DataSourceKind.FireBird,
      DataSourceKind.Sqlite,
      DataSourceKind.LibSql,
      DataSourceKind.Mysql,
      DataSourceKind.MariaDB,
      DataSourceKind.TiDB,
      DataSourceKind.Postgres,
      DataSourceKind.Cockroach,
      DataSourceKind.SqlServer,
      DataSourceKind.DuckDB,
    ].includes(this.kind);
  }

  exportQueryResult(
    queryId: string,
    sql: string,
    kind: ExportKind,
    file: string,
    taskId: string,
    sizeEmitter: Channel<number>,
  ): Promise<void> {
    return commands.exportQueryResultToFile(
      this.uuid,
      taskId,
      sql,
      queryId,
      kind,
      file,
      sizeEmitter,
    );
  }

  setAutoCommit(commit: boolean): Promise<void> {
    return commands.querySetAutocommit(this.uuid, this.queryId, commit);
  }
  getAutoCommit(): Promise<boolean> {
    return commands.queryGetCommit(this.uuid, this.queryId);
  }
  commit(): Promise<void> {
    return commands.queryCommit(this.uuid, this.queryId);
  }
  rollback(): Promise<void> {
    return commands.queryRollback(this.uuid, this.queryId);
  }
  queryOP(sql: string, emit: Channel<QueryEvent>, maxRow: number): Promise<void> {
    return commands.querySql(this.uuid, this.queryId, maxRow, sql, emit);
  }
  executeOP(sql: string): Promise<QueryRespData> {
    return commands.executeSql(this.uuid, this.queryId, sql);
  }

  isDangerSql(sql: string): Promise<boolean> {
    return commands.isDangerousSql(this.uuid, sql);
  }
  destroy() {
    commands
      .querySqlCloseConnection(this.uuid, this.queryId)
      .then(() => {
        console.log("destroy connection of:", this.queryId);
      })
      .catch((e) => {
        console.error("close connection failed,err", e);
      });
  }
}

function modalHandleClose(
  callback: (e: EscDownEvent, reason: "backdropClick" | "escapeKeyDown") => void,
) {
  return (e: EscDownEvent, reason: "backdropClick" | "escapeKeyDown") => {
    if (reason === "backdropClick") {
      return;
    }
    callback(e, reason);
  };
}

function extractBaseName(file: string): string {
  const fileName = file.match(/[^\\/]+$/)?.[0] ?? "";
  return fileName;
}

const keyNotInTable: string = "KWfCwJ62hRf6TKuvMcsQ3eXGtD1tdg2J";

type EscDownEvent = { preventDefault: () => void };

class NameAndQueryId {
  constructor(
    public name: string,
    public kind: DataSourceKind,
    public queryId?: string,
  ) {}
}

type FieldAndGridField = Field & GridColDef;

class PassWordManager {
  static serviceConnection = "connetion.site.dbpal.dev";
  static serviceLLM = "llm.site.dbpal.dev";

  static password = new Map<string, string>();

  static async savePassword(
    service: string,
    name: string,
    pass: string,
    save2KeyRing: boolean,
  ) {
    PassWordManager.password.set(PassWordManager.formatKey(service, name), pass);
    if (save2KeyRing) {
      console.info("save password to keyring", name, pass);
      await keyring.setPassword(service, name, pass);
    }
  }
  static async getPassword(service: string, name: string) {
    const key = PassWordManager.formatKey(service, name);
    if (PassWordManager.password.has(key)) {
      return PassWordManager.password.get(key);
    }
    try {
      return await keyring.getPassword(service, name);
    } catch (e) {
      console.log(
        "get password failed,this is fine maybe this connection don't have password",
        e,
      );
    }
    return null;
  }
  static formatKey(service: string, name: string) {
    return `${service}-${name}`;
  }
  static async deletePassword(service: string, name: string) {
    try {
      PassWordManager.password.delete(PassWordManager.formatKey(service, name));
      return await keyring.deletePassword(service, name);
    } catch (e) {
      console.log("delete password failure", e);
    }
  }
}

async function editorAppendContent(
  editor: editor.IStandaloneCodeEditor,
  text: string,
): Promise<void> {
  const model = editor.getModel();
  const lastLine = model.getLineCount();
  const lastCol = model.getLineMaxColumn(lastLine);
  model.applyEdits([
    {
      range: {
        startLineNumber: lastLine,
        startColumn: lastCol,
        endLineNumber: lastLine,
        endColumn: lastCol,
      },
      text,
      forceMoveMarkers: true,
    },
  ]);
  editor.revealLine(lastLine + 1);
}

export function isSQLCode(codeContent: string, langTag: string = ""): boolean {
  // 1. Check Language Identifier (Highest Priority)
  const sqlTags = new Set([
    "sql",
    "mysql",
    "postgresql",
    "postgres",
    "psql",
    "tsql",
    "plsql",
    "sqlite",
    "mariadb",
    "oracle",
    "hive",
    "presto",
  ]);

  const normalizedTag = langTag.toLowerCase().trim();
  if (sqlTags.has(normalizedTag)) {
    return true;
  }

  // 2. Keyword Pattern Matching (Fall-back for untagged blocks)
  // We use word boundaries (\b) and case-insensitive flags
  const sqlPatterns = [
    /\bSELECT\b.*\bFROM\b/is,
    /\bINSERT\s+INTO\b/is,
    /\bUPDATE\b.*\bSET\b/is,
    /\bDELETE\s+FROM\b/is,
    /\bCREATE\s+(TABLE|VIEW|INDEX|DATABASE|SCHEMA)\b/is,
    /\bALTER\s+TABLE\b/is,
    /\bDROP\s+(TABLE|VIEW)\b/is,
    /\bGROUP\s+BY\b/is,
    /\bORDER\s+BY\b/is,
    /\bLEFT|RIGHT|INNER\s+JOIN\b/is,
  ];

  const isKeywordMatch = sqlPatterns.some((regex) => regex.test(codeContent));
  if (isKeywordMatch) return true;

  // 3. Structural Clues (Comments or Semicolons)
  // Only confirm if a basic SQL verb is also present to avoid false positives with Shell scripts
  const hasSemicolon = /;\s*$/.test(codeContent);
  const hasSqlComments = /--\s+|(\/\*[\s\S]*?\*\/)/.test(codeContent);
  const hasBasicVerb = /\b(SELECT|INSERT|UPDATE|DELETE|CREATE|DROP|GRANT)\b/i.test(
    codeContent,
  );

  if ((hasSemicolon || hasSqlComments) && hasBasicVerb) {
    return true;
  }

  return false;
}

export function isRedisCode(code: string): boolean {
  let codeUpper: string = code.toUpperCase();
  for (const cmd of redisCommands) {
    if (codeUpper.includes(cmd)) {
      return true;
    }
  }
  return false;
}

export function generateRandomId(length = 8) {
  const letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
  const allChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

  // 1. 确保首位是字母
  let result = letters.charAt(Math.floor(Math.random() * letters.length));

  // 2. 生成剩余部分
  for (let i = 1; i < length; i++) {
    result += allChars.charAt(Math.floor(Math.random() * allChars.length));
  }

  return result;
}

const providerQueryID = { id: "" };
export {
  dropDatabase,
  dropSchema,
  editorAppendContent,
  extractBaseName,
  keyNotInTable,
  formatSqlFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  PassWordManager,
  QueryConn,
  reloadOK,
  convertPathToDataUrl,
  uint8ArrayToBase64,
  providerQueryID,
  uuidv4,
};
export type { EscDownEvent, FieldAndGridField };
