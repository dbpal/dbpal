'use strict';
import { SxProps } from '@mui/material';

import { languages, editor, Position, CancellationToken } from 'monaco-editor';
import { types } from './ApiStruct';
import { format as formatSql, SqlLanguage } from 'sql-formatter';
import { createContext } from 'react';
import { loader } from '@monaco-editor/react';
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
  borderRadius: '4px',
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

const getNamedConfig = async (
  name: string
): Promise<types.DBConnectionConfig> => {
  let x = await appStore.get<types.DBConnectionConfig[]>(
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
    let tab = tabs[i];
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
const registeredLanguage = new Set<DataSourceKind>();

const autoCompletion = async (
  modal: editor.ITextModel,
  pos: Position
): Promise<languages.CompletionList> => {
  try {
    let sql = modal.getValue();
    let name = SetAutoCompletionName.get(modal.id);
    if (!name) {
      message("auto completion name is null this is a bug", { kind: 'error' });
      return;
    }
    let completionItem = await invoke<types.CompletionItem[]>(
      'auto_completion',
      {
        sql,
        line: pos.lineNumber,
        col: pos.column,
        name: name.name,
        queryId: name.queryId
      }
    );
    let suggestions = backendCompletionItemToEditor(completionItem);
    return suggestions;
  } catch (e) {
    console.log('completion server error:', e);
    return { suggestions: [] }
  }
};


function initLanguageXXX(
  kind: DataSourceKind,
  ILanguage: languages.IMonarchLanguage,
) {
  loader
    .init()
    .then((monaco) => {
      monaco.languages.register({ id: kind });
      monaco.languages.setMonarchTokensProvider(kind, ILanguage);
      if (kind != DataSourceKind.Redis) {
        monaco.languages.registerDocumentFormattingEditProvider(kind, {
          provideDocumentFormattingEdits(model, options, _token) {
            const text = makeFormatFn(kind)(model.getValue(), options);
            return [{ range: model.getFullModelRange(), text }];
          },
        });
      }
      monaco.languages.registerCompletionItemProvider(kind, {
        provideCompletionItems: (
          model: editor.ITextModel,
          position: Position,
          _context: any,
          _token: CancellationToken
        ) => {
          return autoCompletion(model, position);
        },
        triggerCharacters: ['.'],
      });
    })
    .catch((e) => {
      console.log('init language error:', e);
    });
}

function makeFormatFn(kind: DataSourceKind, showNotification?) {
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
      if (showNotification) {
        showNotification('error', "format failed", e)
      }
      console.log('format sql failed error:', e);
      return sql;
    }
  }
}
// uuid as language identifier.
async function initMonacoLanguage(uuid: string, kind: DataSourceKind, showErr: (x: string) => void) {
  // mongodb don't need this
  if (kind == DataSourceKind.Mongo) {
    return;
  }
  if (registeredLanguage.has(kind)) {
    return;
  }
  registeredLanguage.add(kind);
  let keywordReg;
  try {
    const kw = await invoke<string[]>('key_words', { name: uuid });
    // /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|int8_array|float32_array|user_define|true|false|null|undefined|code)\b/i 
    keywordReg = new RegExp(
      `\\b(${kw.join('|')})\\b`,
      'i'
    );
  } catch (e) {
    showErr("get keyword error:" + e.toString());
    console.log('get keyword error:', e);
    return;
  }
  const singleLineCommentReg = /(--[^\n]*|#[^\n]*)/g;
  const stringReg = /\'(?:\\.|[^\\\'])*\'/;


  switch (kind) {
    case DataSourceKind.MariaDB:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],

              [singleLineCommentReg, 'comment'],
            ],
          },
          brackets: [
            { open: '{', close: '}', token: 'delimiter.curly' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.Mysql:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],
              [/(--[^\n]*|#[^\n]*)/g, 'comment'],
            ],
          },
          brackets: [
            { open: '{', close: '}', token: 'delimiter.curly' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.ClickHouse:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
          brackets: [
            { open: '(', close: ')', token: 'delimiter.parenthesis' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
      );
      break;

    case DataSourceKind.TiDB:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[\$_a-z][\$_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
          brackets: [
            { open: '(', close: ')', token: 'delimiter.parenthesis' },
            { open: '[', close: ']', token: 'delimiter.square' },
          ],
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.Sqlite:
    case DataSourceKind.LibSql:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.FireBird:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [stringReg, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.DuckDB:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z\$][_a-z0-9\$]*/i, 'identifier'],
              [stringReg, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;

    case DataSourceKind.Postgres:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.Cockroach:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\"(?:\\.|[^\"\"])*\"/, 'string'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],

              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.MsSql:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [/\"(?:\\.|[^\\\"])*\"/, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.Redis:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: { root: [[keywordReg, 'keyword']] },
        } as languages.IMonarchLanguage,
      );
      break;
    case DataSourceKind.Oracle:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,

      );
      break;
    case DataSourceKind.Cassandra:
      initLanguageXXX(
        kind,
        {
          ignoreCase: true,
          tokenizer: {
            root: [
              [keywordReg, 'keyword'],
              [/[_a-z][_a-z0-9]*/i, 'identifier'],
              [/\'(?:\\.|[^\'\'])*\'/, 'string'],
              [singleLineCommentReg, 'comment'],
            ],
          },
        } as languages.IMonarchLanguage,
      );
      break;
    default:
      console.log(`impl this ${kind}`);
  }
}

interface IQueryConn {
  queryOP: (text: string, append: boolean) => Promise<any[]>;
  executeOP: (text: string) => Promise<any>;
  reconnect: () => Promise<void>;
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

    let x = invoke<any[]>('query_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
      append
    });
    return x;
  }
  executeOP(sql: string): Promise<any> {
    let x = invoke<any[]>('execute_sql', {
      name: this.uuid,
      queryId: this.queryId,
      sql,
    });
    return x;
  }

  reconnect(): Promise<void> {
    return invoke('query_sql_re_connection', {
      name: this.uuid,
      queryId: this.queryId,
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
    let where_null = where ? where : null;
    let orderBy_null = orderBy ? orderBy : null;
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
    let where_null = where ? where : null;
    let orderBy_null = orderBy ? orderBy : null;
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
  constructor(public name: string, public queryId?: number) {
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

export {
  modalStyle,
  tabsHasId,
  msgTimeout,
  OpenedContext,
  getNamedConfig,
  dropDatabase,
  dropSchema,
  reloadOK,
  initMonacoLanguage,
  QueryConn,
  AppStoreKey,
  keyNotInTable,
  extractBaseName,
  autoCompletion,
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

export type { DefAndMetas, IQueryConn, MakeIQueryConn, EscDownEvent };
