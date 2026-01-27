import types from "./api.ts";

interface CheckConn {
  // milliseconds to check conn
  checkConn: () => Promise<number>;
}

interface QuerySql {
  getAutoCommit: (queryId: string) => Promise<boolean>;
  setAutoCommit: (queryId: string) => Promise<void>;
  commit: (queryId: string) => Promise<void>;
  rollback: (queryId: string) => Promise<void>;
  querySql: (
    queryId: string,
    sql: string,
    emit: () => void,
  ) => Promise<void>;
  executeSql: (queryId: string, sql: string) => Promise<void>;

  executeSqlNoOpenConn: (sql: string) => Promise<void>;

  openConnection: (id: string) => Promise<void>;
  closeConnection: (id: string) => Promise<void>;
}

interface ServerMetas {
  showMetas: () => Promise<types.MetaValue[]>;
}

interface ReadOnly {
  readOnly: () => boolean;
}

interface ExportQueryResult {
  exportQueryResultToFile: (
    queryId: string,
    sql: string,
    kind: types.ExportKind,
    file: string,
  ) => Promise<void>;
}

interface GetBackendKind {
  backendName(): types.BackendKind;
}

interface SchemaTrait {
  duplicateTable: (table: string, dup: string) => Promise<void>;
  listTable: () => Promise<types.ListTable[]>;
  showTableColumns: (table: string) => Promise<types.Field[]>;
  showTable: (
    tableName: string,
    para: types.ShowTableParameter,
  ) => Promise<void>;
  truncateTable: (table: string) => Promise<void>;
  listView: () => Promise<types.ListTable[]>;
  dropView: (name: string) => Promise<void>;
  dropTable: (name: string, cascade: boolean) => Promise<void>;
  listDataTypes(): Promise<string[]>;
  dropTableColumn: (table: string, col: string) => Promise<void>;
  modifyTableColumn: (
    table: string,
    oldName: string,
    newName: string,
    typ?: types.ColumnType,
  ) => Promise<void>;

  addTableColumn: (
    table: string,
    name: string,
    typ: types.ColumnType,
  ) => Promise<void>;

  renameIndex: (
    table: string,
    oldName: string,
    newName: string,
  ) => Promise<void>;
  showIndexes: (table: string) => Promise<types.Index[]>;
  createIndex: (
    table: string,
    req: types.CreateIndexReq,
  ) => Promise<void>;
  dropIndex: (table: string, index: string) => Promise<void>;
  listMaterializedView: () => Promise<types.ListTable[]>;
  dropMaterializedView: (name: string) => Promise<void>;
  showView: (name: string) => Promise<void>;

  exportTableOrViewToFile: (
    name: string,
    para: types.ShowTableParameter | null,
    filepath: string,
    kind: types.ExportKind,
  ) => Promise<void>;
  showCreateTableSql(tableName: string): Promise<types.CreateTableAndIndexSql>;
  showViewSql(viewName: string, materialized: boolean): Promise<string>;
  listTrigger(tableName: string): Promise<types.NamedStaff[]>;
  showTrigger(tableName: string, name: string): Promise<types.TriggerDetail>;
  dropTrigger(tableName: string, name: string): Promise<void>;

  disableTrigger(tableName: string, name: string): Promise<void>;

  enableTrigger(tableName: string, name: string): Promise<void>;

  showUdf(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<types.DefAndMetas>;

  dropUdf(
    name: string,
    extra?: types.UdfOrProcedureExtraParameter,
  ): Promise<void>;

  listProcedure(): Promise<types.NamedStaff[]>;

  showProcedure(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<types.ProcedureDetail>;

  dropProcedure(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<void>;

  tableExists(table: string): Promise<boolean>;

  listCreatedTypes(): Promise<types.CreatedTypeDef[]>;

  dropCreatedType(name: string): Promise<void>;

  exportTableToString(
    tableName: string,
    kind: types.ExportKind,
  ): Promise<string>;

  listSequence(): Promise<types.SequenceDetail[]>;

  renameSequence(oldName: string, newName: string): Promise<void>;

  dropSequence(name: string, cascade: boolean): Promise<void>;

  listSqlCurrentValues(): string[];
}

interface DBTrait
  extends QuerySql, CheckConn, ServerMetas, ReadOnly, ExportQueryResult {
  listSchema(): Promise<string[]>;
  openSchema(name: string): Promise<SchemaTrait>;
  createSchema(name: string): Promise<void>;
  dropSchema(name: string): Promise<void>;
}

export type {
  CheckConn,
  DBTrait,
  ExportQueryResult,
  QuerySql,
  SchemaTrait,
  ServerMetas,
};
