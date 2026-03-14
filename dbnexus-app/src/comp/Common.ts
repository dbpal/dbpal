import { SxProps } from "@mui/material";
import { editor, languages, Position } from "monaco-editor";
import { types } from "./ApiStruct.ts";
import { format as formatSql, SqlLanguage } from "sql-formatter";
import { createContext } from "react";
import { TabContent } from "./Tabs.tsx";
import { Channel, invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import * as keyring from "tauri-plugin-keyring-api";
import {
  appDatabase,
  backendCompletionItemToEditor,
  DataSourceKind,
  extractConfigNameFromUuid,
  loader,
  MuiDataGridData,
  QueryRespData,
  secondConfirm,
} from "./helper.ts";
import { GridColDef } from "@mui/x-data-grid";

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

class Connection {
  id: number;
  name: string;
  config: string;
}

class ConnectionHandler {
  static async getByName(name: string): Promise<types.DBConnectionConfig> {
    try {
      const x = await appDatabase.select<Connection[]>(
        "select * from connection where name=?",
        [name],
      );
      if (x.length != 1) {
        message("database should return one result", { kind: "error" });
        return null;
      }
      const c = JSON.parse(x[0].config);
      return c as types.DBConnectionConfig;
    } catch (e) {
      message(`get connection failed`, { kind: "error" });
      console.error("get connection failed,err:", e);
    }
    return null;
  }
  static async deleteConn(name: string) {
    return appDatabase.execute("delete from connection where name = ?", [name]);
  }
  static async getAll(): Promise<types.DBConnectionConfig[]> {
    const c = await appDatabase.select<Connection[]>(
      "select * from connection order by id desc",
    );
    return c.map((x) => JSON.parse(x.config));
  }
  static async save(config: types.DBConnectionConfig) {
    return appDatabase.execute(
      "insert into connection(name,config) values(?,?)",
      [
        config.name,
        JSON.stringify(config),
      ],
    );
  }

  static nameDuplicated(name: string): Promise<boolean> {
    return appDatabase
      .select("select count(*) as count from connection where name=?", [name])
      .then((x) => x[0].count != 0);
  }
}

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
  invoke("drop_database", { name: uuid, database })
    .then(() => {
      showNotification("success", "drop ok");
      if (okCallback) {
        okCallback();
      }
    })
    .catch((e) => {
      showNotification("error", "drop failed", e);
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
  invoke("drop_schema", { name: uuid, schema })
    .then(() => {
      showNotification("success", "Drop OK");
      okCallback();
    })
    .catch((e) => {
      showNotification("error", "Drop failure", e);
    });
}

function tabsHasId(tabs: TabContent[], id: string): number | null {
  for (let i = 0; i < tabs.length; i++) {
    const tab = tabs[i];
    if (tab.id === id) {
      return i;
    }
  }
  return null;
}

interface DefAndMetas {
  def: string;
  metas?: types.MetaValue[];
}

const OpenedContext = createContext<{ opened: boolean; setOpened: any }>(null);

function reloadOK(showNotification) {
  showNotification("success", "data loaded.");
}

const autoCompletion = async (
  modal: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList> => {
  try {
    const sql = modal.getValue();
    const name = SetAutoCompletionName.get(modal.id);
    if (!name) {
      console.error(
        "editor id map to backend name is nul,this is internal bug",
      );
      return;
    }
    const completionItem = await invoke<types.CompletionList>(
      "auto_completion",
      {
        sql,
        line: pos.lineNumber,
        col: pos.column,
        name: name.name,
        queryId: name.queryId,
      },
    );
    const suggestions = backendCompletionItemToEditor(completionItem.items);
    suggestions.incomplete = completionItem.incomplete;
    return suggestions;
  } catch (e) {
    console.error("completion server error:", e);
    return { suggestions: [] };
  }
};

const aiTextAutoCompletion = async (
  modal: editor.ITextModel,
  pos: Position,
): Promise<languages.CompletionList> => {
  try {
    const text = modal.getValue();
    {
      // try if this is ai text completion
      const name = AITextCompletionName.get(modal.id);
      if (name) {
        const completionItem = await invoke<types.CompletionList>(
          "ai_text_completion",
          {
            text,
            line: pos.lineNumber,
            col: pos.column,
            name: name.name,
            queryId: name.queryId,
          },
        );
        const suggestions = backendCompletionItemToEditor(completionItem.items);
        suggestions.incomplete = completionItem.incomplete;
        return suggestions;
      }
    }
    console.info("nothing to complete");
    return { suggestions: [] };
  } catch (e) {
    console.error("completion server error:", e);
    return { suggestions: [] };
  }
};

loader.init().then((monaco) => {
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
      const name = SetAutoCompletionName.get(model.id);
      if (!name) {
        console.error(
          "editor id map to backend name is nul,this is internal bug",
        );
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
  open(): Promise<void> {
    console.log("open connection", this.uuid, this.queryId);
    return invoke("query_sql_open_connection", {
      name: this.uuid,
      id: this.queryId,
    });
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
    kind: types.ExportKind,
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
  queryOP(
    sql: string,
    emit: Channel<QueryEvent>,
    maxRow: number,
  ): Promise<void> {
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
    return invoke("is_dangerous_sql", { name: this.uuid, sql });
  }
  destroy() {
    invoke("query_sql_close_connection", {
      name: this.uuid,
      queryId: this.queryId,
    })
      .then(() => {
        console.log("destroy connection of:", this.queryId);
      })
      .catch((e) => {
        console.error("close connection failed,err", e);
      });
  }
}

function modalHandleClose(
  callback: (
    e: EscDownEvent,
    reason: "backdropClick" | "escapeKeyDown",
  ) => void,
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
class SetAutoCompletionName {
  static id2Name = new Map<string, NameAndQueryId>();
  static set(editorId: string, x: NameAndQueryId) {
    SetAutoCompletionName.id2Name.set(editorId, x);
  }

  static get(editorId: string): NameAndQueryId | null {
    return SetAutoCompletionName.id2Name.get(editorId);
  }
}

class AITextCompletionName {
  static id2Name = new Map<string, string>();
  static set(editorID: string, queryEditorID: string) {
    AITextCompletionName.id2Name.set(editorID, queryEditorID);
  }

  static get(editorId: string): NameAndQueryId | null {
    let id = AITextCompletionName.id2Name.get(editorId);
    if (!id) {
      return null;
    }
    return SetAutoCompletionName.get(id);
  }
}

type FieldAndGridField = types.Field & GridColDef;

class PassWordManager {
  static service = "site.dbpal.dev";
  static password = new Map<string, string>();
  static async savePassword(name: string, pass: string, save2KeyRing: boolean) {
    PassWordManager.password.set(name, pass);
    if (save2KeyRing) {
      console.info("save password to keyring", name, pass);
      await keyring.setPassword(PassWordManager.service, name, pass);
    }
  }
  static async getPassword(name: string) {
    if (PassWordManager.password.has(name)) {
      return PassWordManager.password.get(name);
    }
    try {
      return await keyring.getPassword(PassWordManager.service, name);
    } catch (e) {
      console.log(
        "get password failed,this is fine maybe this connection don't have password",
        e,
      );
    }
    return null;
  }
  static async deletePassword(name: string) {
    try {
      PassWordManager.password.delete(name);
      return await keyring.deletePassword(PassWordManager.service, name);
    } catch (e) {
      console.log("delete password failure", e);
    }
  }
}

const connectionChangeEvent = "connection-has-changged";
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

interface TableFilter {
  filter: string;
}

class TableFilterHandler {
  static async get(uuid: string, name: string) {
    return appDatabase.select<TableFilter[]>(
      "select filter from table_filter where uuid=? and name=?",
      [uuid, name],
    );
  }

  static async add(uuid: string, name: string, filter: string) {
    await appDatabase.execute(
      "replace into table_filter (uuid,connection_name,name,filter,last_modified) values (?,?,?,?,?)",
      [uuid, extractConfigNameFromUuid(uuid), name, filter, new Date()],
    );
    let count = (
      await appDatabase.select(
        "select count(*) as count  from table_filter where uuid=? and name=?",
        [uuid, name],
      )
    )[0].count;
    if (count > 5) {
      await appDatabase.execute(
        "delete from table_filter where uuid =? and name=? order by last_modified limit 1",
        [uuid, name],
      );
    }
  }
  static async deleteByConnectionName(name: string) {
    try {
      await appDatabase.execute(
        "delete from table_filter where connection_name=?",
        [
          name,
        ],
      );
    } catch (e) {
      console.error("delete table filter failed,err:", e);
      message("delete table filter failed,err:" + String(e), { kind: "error" });
    }
  }
}

export {
  AITextCompletionName,
  connectionChangeEvent,
  ConnectionHandler,
  dropDatabase,
  dropSchema,
  editorAppendContent,
  extractBaseName,
  keyNotInTable,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  OpenedContext,
  PassWordManager,
  QueryConn,
  reloadOK,
  SetAutoCompletionName,
  TableFilterHandler,
  tabsHasId,
};

export type {
  DefAndMetas,
  EscDownEvent,
  FieldAndGridField,
  QueryEvent,
  TableFilter,
};
