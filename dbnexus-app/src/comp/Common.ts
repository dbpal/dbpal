"use strict";
import { SxProps } from "@mui/material";
import { languages, editor, Position } from "monaco-editor";
import { types } from "./ApiStruct";
import { format as formatSql, SqlLanguage } from "sql-formatter";
import { createContext } from "react";
import { TabContent } from "./Tabs";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import * as keyring from "tauri-plugin-keyring-api";
import {
  appDatabase,
  AppStoreKey,
  backendCompletionItemToEditor,
  DataSourceKind,
  secondConfirm,
} from "./helper";
import { GridColDef } from "@mui/x-data-grid";
import { loader } from "@monaco-editor/react";

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
    return c.map(x => JSON.parse(x.config));
  }
  static async save(config: types.DBConnectionConfig) {
    return appDatabase.execute("insert into connection(name,config) values(?,?)", [
      config.name,
      JSON.stringify(config),
    ]);
  }

  static nameDuplicated(name: string): Promise<boolean> {
    return appDatabase
      .select("select count(*) as count from connection where name=?", [name])
      .then(x => x[0].count != 0);
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
    .catch(e => {
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
    .catch(e => {
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
      console.error("editor id map to backend name is nul,this is internal bug");
      return;
    }
    const completionItem = await invoke<types.CompletionList>("auto_completion", {
      sql,
      line: pos.lineNumber,
      col: pos.column,
      name: name.name,
      queryId: name.queryId,
    });
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

loader.init().then(monaco => {
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

interface IQueryConn {
  queryOP: (text: string, append: boolean) => Promise<any[]>;
  executeOP: (text: string) => Promise<any>;
  reconnect: () => Promise<IQueryConn>;
  exportQueryResult: (
    sql: string,
    kind: types.ExportKind,
    file: string,
    taskId: string,
  ) => Promise<void>;
  isDangerSql: (sql: string) => Promise<boolean>;
  destroy: () => void;
  setAutoCommit: (boolean) => Promise<void>;
  getAutoCommit: () => Promise<boolean>;
  commit: () => Promise<void>;
  rollback: () => Promise<void>;
  supportTransaction: () => boolean;
  queryId: number;
}

class QueryConn implements IQueryConn {
  queryId: number;
  uuid: string;
  kind: DataSourceKind;

  constructor(uuid, queryId: number, kind: DataSourceKind) {
    this.queryId = queryId;
    this.uuid = uuid;
    this.kind = kind;
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
    ].includes(this.kind);
  }

  exportQueryResult(
    sql: string,
    kind: types.ExportKind,
    file: string,
    taskId: string,
  ): Promise<void> {
    return invoke<void>("export_query_result_to_file", {
      name: this.uuid,
      sql,
      kind,
      file,
      taskId,
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
  queryOP(sql: string, append: boolean): Promise<any[]> {
    return invoke<any[]>("query_sql", {
      name: this.uuid,
      queryId: this.queryId,
      sql,
      append,
    });
  }
  executeOP(sql: string): Promise<any> {
    return invoke<any[]>("execute_sql", {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
  }

  reconnect(): Promise<IQueryConn> {
    return invoke<number>("query_sql_re_connection", {
      name: this.uuid,
      queryId: this.queryId,
    }).then(queryid => {
      return new QueryConn(this.uuid, queryid, this.kind);
    });
  }
  isDangerSql(sql: string): Promise<boolean> {
    return invoke("is_dangerous_sql", { name: this.uuid, sql });
  }
  destroy() {
    console.log("destroy connection of:", this.queryId);
    invoke("query_sql_close_connection", {
      name: this.uuid,
      queryId: this.queryId,
    })
      .then(() => {})
      .catch(e => {
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

type MakeIQueryConn = () => Promise<IQueryConn>;
const keyNotInTable = "KWfCwJ62hRf6TKuvMcsQ3eXGtD1tdg2J";

type EscDownEvent = { preventDefault: () => void };

class NameAndQueryId {
  constructor(
    public name: string,
    public kind: DataSourceKind,
    public queryId?: number,
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

const makeQueryConn = async (uuid: string, kind: DataSourceKind) => {
  const queryId = await invoke<number>("query_sql_open_connection", {
    name: uuid,
  });
  console.info("open connection:", queryId);
  const ret = new QueryConn(uuid, queryId, kind);
  return ret;
};

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

export {
  modalStyle,
  tabsHasId,
  OpenedContext,
  ConnectionHandler,
  dropDatabase,
  dropSchema,
  reloadOK,
  AppStoreKey,
  keyNotInTable,
  extractBaseName,
  modalHandleClose,
  SetAutoCompletionName,
  NameAndQueryId,
  PassWordManager,
  connectionChangeEvent,
  makeQueryConn,
  editorAppendContent,
  AITextCompletionName,
  makeFormatFn,
};

export type {
  DefAndMetas,
  IQueryConn,
  MakeIQueryConn,
  EscDownEvent,
  FieldAndGridField,
  QueryConn,
};
