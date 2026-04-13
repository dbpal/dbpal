import { SxProps } from "@mui/material";
import { editor, languages, Position } from "monaco-editor";
import { format as formatSql, SqlLanguage } from "sql-formatter";
import { Channel, invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import * as keyring from "tauri-plugin-keyring-api";
import {
  backendCompletionItemToEditor,
  DataSourceKind,
  MuiDataGridData,
  QueryRespData,
  secondConfirm,
} from "./helper.ts";
import { GridColDef } from "@mui/x-data-grid";
import * as monaco from "monaco-editor";
import { readFile, stat } from "@tauri-apps/plugin-fs";
import { filesize } from "filesize";
import { redisCommands } from "./redisCmd.ts";
import {
  commands,
  ExportKind,
  Field,
  MetaValue,
  NamedStaff,
  UdfOrProcedureExtraParameter,
} from "./bindings.ts";

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
  animation: "modalAnimation 0.6s ease-out",
};

async function dropDatabase(props: {
  uuid: string;
  database;
  showNotification;
  okCallback?;
}) {
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

async function dropSchema({ uuid, schema, showNotification, okCallback }) {
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

interface DefAndMetas {
  def: string;
  metas?: MetaValue[];
}

function reloadOK(showNotification) {
  showNotification({ severity: "success", title: "data loaded." });
}

const dbpalQuerykey = "_dbpal_query_id_xxx_yyy_zzz";
const autoCompletion = async (
  model: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList> => {
  const sql = model.getValue();
  const name = model[dbpalQuerykey] as NameAndQueryId;
  if (!name) {
    console.error("editor id map to backend name is nul,this is internal bug");
    return;
  }
  const result = await commands.autoCompletion(
    name.queryId,
    name.queryId,
    sql,
    pos.lineNumber,
    pos.column,
  );
  if (result.status == "ok") {
    const suggestions = backendCompletionItemToEditor(result.data.items);
    suggestions.incomplete = result.data.incomplete;
    return suggestions;
  } else {
    console.error("completion server error:", result.error);
    return { suggestions: [] };
  }
};

const aiTextAutoCompletion = async (
  modal: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList> => {
  const text = modal.getValue();
  {
    // try if this is ai text completion
    const name = modal[dbpalQuerykey];
    if (name) {
      const result = await commands.aiTextCompletion(
        name.name,
        text,
        pos.lineNumber,
        pos.column,
      );

      if (result.status == "ok") {
        const suggestions = backendCompletionItemToEditor(result.data.items);
        suggestions.incomplete = result.data.incomplete;
        return suggestions;
      } else {
        console.error("completion server error:", result.error);
        return { suggestions: [] };
      }
    }
  }
  console.info("nothing to complete");
  return { suggestions: [] };
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
    const name = model[dbpalQuerykey];
    if (!name) {
      console.error("editor id map to backend name is nul,this is internal bug");
      return;
    }
    const text = makeFormatFn(name.kind)(model.getValue(), {
      insertSpaces: options.insertSpaces,
      tabSize: options.tabSize,
    });
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
function makeFormatFn(kind: DataSourceKind) {
  return (sql: string, opts?: languages.FormattingOptions) => {
    if (!opts) {
      opts = {
        tabSize: 2,
        insertSpaces: true,
      };
    }
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
  };
}

type QueryEvent =
  | ({ kind: "structure" } & { resp: MuiDataGridData })
  | ({ kind: "err" } & { err: string })
  | ({ kind: "data" } & { data: string[][] })
  | ({ kind: "status" } & { status: string })
  | { kind: "end" };

class QueryConn {
  queryId: string;
  uuid: string;
  kind: DataSourceKind;

  constructor(uuid, queryId: string, kind: DataSourceKind) {
    this.queryId = queryId;
    this.uuid = uuid;
    this.kind = kind;
  }
  async open(): Promise<void> {
    console.log("open connection", this.uuid, this.queryId);
    let r = await commands.querySqlOpenConnection(this.uuid, this.queryId);
    if (r.status == "error") {
      throw r.error;
    }
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
    return invoke<void>("export_query_result_to_file", {
      name: this.uuid,
      sql,
      kind,
      file,
      taskId,
      sizeEmitter,
      queryId,
    });
  }

  setAutoCommit(commit: boolean): Promise<void> {
    return invoke<void>("query_set_autocommit", {
      name: this.uuid,
      queryId: this.queryId,
      commit,
    });
  }
  getAutoCommit(): Promise<boolean> {
    return invoke<boolean>("query_get_commit", {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  commit(): Promise<void> {
    return invoke<void>("query_commit", {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  rollback(): Promise<void> {
    return invoke<void>("query_rollback", {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  queryOP(sql: string, emit: Channel<QueryEvent>, maxRow: number): Promise<void> {
    return invoke<void>("query_sql", {
      name: this.uuid,
      queryId: this.queryId,
      sql,
      emit,
      maxRow,
    });
  }
  executeOP(sql: string): Promise<QueryRespData> {
    return invoke<QueryRespData>("execute_sql", {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
  }

  isDangerSql(sql: string): Promise<boolean> {
    return commands
      .isDangerousSql(this.uuid, sql)
      .then((e) => (e.status == "ok" ? e.data : false));
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

function extractBaseName(file: string) {
  const fileName = file.match(/[^\\/]+$/)[0];
  return fileName;
}

const keyNotInTable = "KWfCwJ62hRf6TKuvMcsQ3eXGtD1tdg2J";

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

async function editorAppendContent(editor: editor.IStandaloneCodeEditor, text) {
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

export function isRedisCode(code: string) {
  let codeUpper = code.toUpperCase();
  for (const cmd of redisCommands) {
    if (codeUpper.includes(cmd)) {
      return true;
    }
  }
  return false;
}
function makeUdfExtraArg(
  udf: NamedStaff,
  kind: DataSourceKind,
): UdfOrProcedureExtraParameter {
  let arg = null;
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      arg = { extra: "postgres", parameters: udf.parameters };
      break;
    case DataSourceKind.Cassandra:
      arg = { extra: "cassandra", parameters: udf.cassandra_parameters };
      break;
  }
  return arg;
}
export {
  dbpalQuerykey,
  dropDatabase,
  dropSchema,
  editorAppendContent,
  extractBaseName,
  keyNotInTable,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  PassWordManager,
  QueryConn,
  reloadOK,
  convertPathToDataUrl,
  uint8ArrayToBase64,
  makeUdfExtraArg,
};
export type { DefAndMetas, EscDownEvent, FieldAndGridField, QueryEvent };

/* Tauri Specta runtime */
export async function unwrapCommand<T, E>(
  result: Promise<T>,
): Promise<{ status: "ok"; data: T } | { status: "error"; error: E }> {
  try {
    return { status: "ok", data: await result };
  } catch (e) {
    if (e instanceof Error) throw e;
    return { status: "error", error: e as any };
  }
}
