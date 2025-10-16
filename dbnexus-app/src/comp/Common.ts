'use strict';
import { SxProps } from '@mui/material';

import { languages, editor, Position, CancellationToken } from 'monaco-editor';
import { types } from './ApiStruct';
import { format as formatSql, SqlLanguage } from 'sql-formatter';
import { createContext } from 'react';
import { TabContent } from './Tabs';
import { invoke } from '@tauri-apps/api/core';
import { message } from '@tauri-apps/plugin-dialog';
const modalStyle: SxProps = {
  position: 'fixed',
  transform: 'translate(50%, 50%)',
  bottom: '50%',
  right: '50%',
  bgcolor: 'background.paper',
  border: '2px solid skyblue',
  boxShadow: 24,
  borderRadius: '0.4rem',
  p: 1,
  maxHeight: '85vh',
  minHeight: '10vh',
  minWidth: '20vw',
  maxWidth: '80vw',
  display: 'flex',
  overflow: 'auto',
  flexDirection: 'column',
  animation: 'modalAnimation 0.6s ease-out',
};

import {
  appStore,
  AppStoreKey,
  backendCompletionItemToEditor,
  DataSourceKind,
  MuiDataGridData,
  secondConfirm,
} from './helper';
import { GridColDef } from '@mui/x-data-grid';

const getNamedConfig = async (
  name: string
): Promise<types.DBConnectionConfig> => {
  const x = await appStore.get<types.DBConnectionConfig[]>(
    AppStoreKey.ConnectionConfig
  );
  if (!x) {
    return undefined;
  }
  return x.find((x) => x.name == name);
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
    confirmText: 'Drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_database', { name: uuid, database })
    .then(() => {
      showNotification('success', 'Drop OK');
      if (okCallback) {
        okCallback();
      }
    })
    .catch((e) => {
      showNotification('error', 'Drop failed', e);
    });
}

async function dropSchema({ uuid, schema, showNotification, okCallback }) {
  const confirm = await secondConfirm({
    title: `drop '${schema}'`,
    confirmText: 'Drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_schema', { name: uuid, schema })
    .then(() => {
      showNotification('success', 'Drop OK');
      okCallback();
    })
    .catch((e) => {
      showNotification('error', 'Drop failure', e);
    });
}

const msgTimeout = 10000;

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

const OpenedContext = createContext<{ opened: boolean; setOpened }>(null);

function reloadOK(showNotification) {
  showNotification('success', 'data loaded.');
}

const autoCompletion = async (
  modal: editor.ITextModel,
  pos: Position
): Promise<languages.CompletionList> => {
  try {
    const sql = modal.getValue();
    alert(sql);
    const name = SetAutoCompletionName.get(modal.id);
    if (!name) {
      message("auto completion name is null this is a bug", { kind: 'error' });
      return;
    }
    const completionItem = await invoke<types.CompletionItem[]>(
      'auto_completion',
      {
        sql,
        line: pos.lineNumber,
        col: pos.column,
        name: name.name,
        queryId: name.queryId
      }
    );
    const suggestions = backendCompletionItemToEditor(completionItem);
    return suggestions;
  } catch (e) {
    console.log('completion server error:', e);
    return { suggestions: [] }
  }
};
languages.register({ id: 'sql' });


languages.registerCompletionItemProvider(
  { language: 'sql' },
  {
    triggerCharacters: ['.', 'a'],
    provideCompletionItems(model,
      position: Position,
      _context: any,
      _token: CancellationToken
    ) {
      return autoCompletion(model, position);
    },
  }
);
languages.registerDocumentFormattingEditProvider('sql', {
  provideDocumentFormattingEdits(model, options, _) {
    const name = SetAutoCompletionName.get(model.id);
    if (!name) {
      message("auto completion name is null this is a bug", { kind: 'error' });
      return;
    }
    let text = makeFormatFn(name.kind)(model.getValue(), { insertSpaces: options.insertSpaces, tabSize: options.tabSize });
    if (!text) {
      return null
    }
    return [{ range: model.getFullModelRange(), text }]
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
      let language: SqlLanguage = 'sql';
      switch (kind) {
        case DataSourceKind.DuckDB:
          language = 'duckdb';
          break;
        case DataSourceKind.Mysql:
          language = 'mysql';
          break;
        case DataSourceKind.MariaDB:
          language = 'mariadb';
          break;
        case DataSourceKind.TiDB:
          language = 'tidb';
          break;
        case DataSourceKind.Oracle:
          language = 'plsql';
          break;
        case DataSourceKind.Postgres:
        case DataSourceKind.Cockroach:
          language = 'postgresql';
          break;
        case DataSourceKind.LibSql:
        case DataSourceKind.Sqlite:
          language = 'sqlite';
          break;
      }
      return formatSql(sql, {
        language,
        tabWidth: opts.tabSize,
        useTabs: !opts.insertSpaces,
      });
    } catch (e) {
      message('format failed error:' + e.toString(), { kind: 'error' })
      console.log('format sql failed error:', e);
      return sql;
    }
  }
}


interface IQueryConn {
  queryOP: (text: string, append: boolean) => Promise<any[]>;
  executeOP: (text: string) => Promise<any>;
  reconnect: () => Promise<IQueryConn>;
  exportQueryResult: (
    sql: string,
    kind: types.ExportKind,
    file: string
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
      DataSourceKind.MsSql,
    ].includes(this.kind);
  }

  exportQueryResult(
    sql: string,
    kind: types.ExportKind,
    file: string
  ): Promise<void> {
    return invoke<void>('export_query_result_to_file', {
      name: this.uuid,
      sql,
      kind,
      file,
    });
  }

  setAutoCommit(commit: boolean): Promise<void> {
    return invoke<void>('query_set_autocommit', {
      name: this.uuid,
      queryId: this.queryId,
      commit,
    });
  }
  getAutoCommit(): Promise<boolean> {
    return invoke<boolean>('query_get_commit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  commit(): Promise<void> {
    return invoke<void>('query_commit', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  rollback(): Promise<void> {
    return invoke<void>('query_rollback', {
      name: this.uuid,
      queryId: this.queryId,
    });
  }
  queryOP(sql: string, append: boolean): Promise<any[]> {
    return invoke<any[]>('query_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
      append
    });

  }
  executeOP(sql: string): Promise<any> {
    return invoke<any[]>('execute_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
  }

  reconnect(): Promise<IQueryConn> {
    return invoke<number>('query_sql_re_connection', {
      name: this.uuid,
      queryId: this.queryId,
    }).then(queryid => {
      return new QueryConn(this.uuid, queryid, this.kind);
    });
  }
  isDangerSql(sql: string): Promise<boolean> {
    return invoke('is_dangerous_sql', { name: this.uuid, sql });
  }
  destroy() {
    console.log('destroy connection of:', this.queryId);
    invoke('query_sql_close_connection', {
      name: this.uuid,
      queryId: this.queryId,
    })
      .then(() => { })
      .catch((e) => {
        console.log('close connection failed,err', e);
      });
  }
}


function modalHandleClose(callback: (e: EscDownEvent, reason: 'backdropClick' | 'escapeKeyDown') => void) {
  return (e: EscDownEvent, reason: 'backdropClick' | 'escapeKeyDown') => {
    if (reason === 'backdropClick') {
      return;
    }
    callback(e, reason)
  }
}

function extractBaseName(file) {
  const fileName = file.match(/[^\\/]+$/)[0];
  return fileName;
}

type MakeIQueryConn = () => Promise<IQueryConn>;
const keyNotInTable = 'KWfCwJ62hRf6TKuvMcsQ3eXGtD1tdg2J';

type EscDownEvent = { preventDefault: () => void };



function makeExtractTableSelectWhere(uuid) {
  return (sql: string) => {
    return invoke<string | null>('extract_table_select_selection', { name: uuid, sql })
  }
}

function makeExtractTableSelectSort(uuid) {
  return (sql: string) => {
    return invoke<string | null>('extract_table_select_sort', { name: uuid, sql })
  }
}



function makeShowTableFn(uuid: string, table: string) {
  return async (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number,
    extra
  ) => {
    const where_null = where ? where : null;
    const orderBy_null = orderBy ? orderBy : null;
    return await invoke<MuiDataGridData>('show_table', {
      name: uuid,
      tableName: table,
      whereStr: where_null,
      orderBy: orderBy_null,
      page,
      pageSize,
      extra,
    });
  }
}


function makeShowViewFn(uuid: string, view: string) {
  return async (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number,
    extra
  ) => {
    const where_null = where ? where : null;
    const orderBy_null = orderBy ? orderBy : null;
    return await invoke<MuiDataGridData>('show_view', {
      name: uuid,
      viewName: view,
      whereStr: where_null,
      orderBy: orderBy_null,
      page,
      pageSize,
      extra,
    });
  }
}

function makeTableUpdate(uuid, table) {
  return async (wheres, values) => {
    return await invoke<types.SqlExecuteRet>('table_update', {
      name: uuid,
      tableName: table,
      values,
      wheres,
    });
  }
}

function makeTableDelete(uuid, table) {
  return async (wheres) => {
    return await invoke<types.SqlExecuteRet>('table_delete', {
      name: uuid,
      tableName: table,
      wheres,
    });
  }
}

function makeInvalidCassandraTableCache(uuid, table) {
  return async () => {
    return invoke<void>('cassandra_invalid_table_or_view_caching', {
      name: uuid,
      tableName: table,
    });
  }
}


function makePutValueInSql(uuid) {
  return async (typ, value) => {
    return invoke<string>('put_value_in_sql', {
      name: uuid,
      typ,
      value: value ? value : null,
    });
  }
}

class NameAndQueryId {
  constructor(public name: string, public kind: DataSourceKind, public queryId?: number) {
  }
}
class SetAutoCompletionName {

  static id2Name = new Map<string, NameAndQueryId>();
  static set(editorId: string, x: NameAndQueryId) {
    SetAutoCompletionName.id2Name.set(editorId, x)
  }

  static get(editorId: string): NameAndQueryId | null {
    return SetAutoCompletionName.id2Name.get(editorId)
  }
}

function makeShowTableColumn(uuid, table) {
  return () => {
    return invoke<types.Field[]>('show_table_columns', {
      name: uuid,
      tableName: table,
    })
  }
}

function makeTableInsertFn(uuid, table) {
  return async (values) => {
    return invoke<void>('table_insert', {
      name: uuid,
      tableName: table,
      values,
    });
  }
}

type FieldAndGridField = types.Field & GridColDef;

export {
  modalStyle,
  tabsHasId,
  msgTimeout,
  OpenedContext,
  getNamedConfig,
  dropDatabase,
  dropSchema,
  reloadOK,
  QueryConn,
  AppStoreKey,
  keyNotInTable,
  extractBaseName,
  modalHandleClose,
  SetAutoCompletionName,
  NameAndQueryId,
  makeExtractTableSelectWhere,
  makeExtractTableSelectSort,
  makeFormatFn,
  makeShowTableFn,
  makeShowViewFn,
  makeTableUpdate,
  makeTableDelete,
  makeInvalidCassandraTableCache,
  makePutValueInSql,
  makeShowTableColumn,
  makeTableInsertFn,
};

export type { DefAndMetas, IQueryConn, MakeIQueryConn, EscDownEvent, FieldAndGridField };
