import RetryAgent from "asset:///node/undici/retry-agent.d.ts";
import types from "./api.ts";
import { DBTrait, SchemaTrait } from "./interfaces.ts";

import { connect, Connection } from "npm:odbc";

class DB2Server implements SchemaTrait, DBTrait {
  async listTableImpl(schema: string): Promise<types.ListTable[]> {
    const sql = `SELECT TABNAME 
FROM SYSCAT.TABLES 
WHERE TABSCHEMA = '${schema}';
`;
    return await this.conn.query(sql).then((rows) =>
      rows.map((row) => ({
        name: row.TABNAME,
      }))
    );
  }

  constructor(public conn: Connection, public config: types.Db2Config) {}

  static async connect(config: types.Db2Config) {
    const connStr =
      `Driver=${config.driver_name};Hostname=${config.host};Port=${config.port};Protocol=TCPIP;UID=${config.username};PWD=${config.password};CodePage=1208;Database=${config.db};`;
    const conn = await connect(connStr);

    if (config.schema) {
      const sql = `SET SCHEMA ${config.schema};`;
      await conn.query(sql);
    }
    return new DB2Server(conn, config);
  }
  async duplicateTable(table: string, dup: string): Promise<void> {
    const sql =
      `CREATE TABLE ${dup} AS (SELECT * FROM ${table}) DEFINITION ONLY;`;
    await this.conn.query(sql);
  }

  async listTable(): Promise<types.ListTable[]> {
    return this.listTableImpl(this.config.schema as string);
  }

  async showTableColumns(table: string): Promise<types.Field[]> {
    throw new Error("showTableColumns method not implemented.");
  }

  async showTable(
    tableName: string,
    para: types.ShowTableParameter,
  ): Promise<void> {
    throw new Error("showTable method not implemented.");
  }

  async truncateTable(table: string): Promise<void> {
    const sql = `TRUNCATE TABLE ${table}`;
    await this.conn.query(sql);
  }

  async listView(): Promise<types.ListTable[]> {
    throw new Error("listView method not implemented.");
  }

  async dropView(name: string): Promise<void> {
    const sql = `DROP VIEW ${name}`;
    this.conn.query(sql);
  }

  async dropTable(name: string, _cascade: boolean): Promise<void> {
    const sql = `DROP TABLE ${table}`;
    await this.conn.query(sql);
  }

  async listDataTypes(): Promise<string[]> {
    throw new Error("Method not implemented.");
  }

  async dropTableColumn(table: string, col: string): Promise<void> {
    throw new Error("dropTableColumn method not implemented.");
  }

  async modifyTableColumn(
    table: string,
    oldName: string,
    newName: string,
    typ?: types.ColumnType,
  ): Promise<void> {
    throw new Error("modifyTableColumn method not implemented.");
  }

  async addTableColumn(
    table: string,
    name: string,
    typ: types.ColumnType,
  ): Promise<void> {
    throw new Error("addTableColumn method not implemented.");
  }

  async renameIndex(
    table: string,
    oldName: string,
    newName: string,
  ): Promise<void> {
    throw new Error("renameIndex method not implemented.");
  }

  async showIndexes(table: string): Promise<types.Index[]> {
    throw new Error("showIndexes method not implemented.");
  }

  async createIndex(table: string, req: types.CreateIndexReq): Promise<void> {
    throw new Error("createIndex method not implemented.");
  }

  async dropIndex(table: string, index: string): Promise<void> {
    throw new Error("dropIndex method not implemented.");
  }

  async listMaterializedView(): Promise<types.ListTable[]> {
    throw new Error("listMaterializedView method not implemented.");
  }

  async dropMaterializedView(name: string): Promise<void> {
    throw new Error("dropMaterializedView method not implemented.");
  }

  async showView(name: string): Promise<void> {
    throw new Error("showView method not implemented.");
  }

  async exportTableOrViewToFile(
    name: string,
    para: types.ShowTableParameter | null,
    filepath: string,
    kind: types.ExportKind,
  ): Promise<void> {
    throw new Error("exportTableOrViewToFile method not implemented.");
  }

  async showCreateTableSql(
    tableName: string,
  ): Promise<types.CreateTableAndIndexSql> {
    throw new Error("Method not implemented.");
  }

  async showViewSql(viewName: string, materialized: boolean): Promise<string> {
    throw new Error("Method not implemented.");
  }

  async listTrigger(tableName: string): Promise<types.NamedStaff[]> {
    throw new Error("Method not implemented.");
  }

  async showTrigger(
    tableName: string,
    name: string,
  ): Promise<types.TriggerDetail> {
    throw new Error("Method not implemented.");
  }

  async dropTrigger(tableName: string, name: string): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async disableTrigger(tableName: string, name: string): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async enableTrigger(tableName: string, name: string): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async showUdf(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<types.DefAndMetas> {
    throw new Error("Method not implemented.");
  }

  async dropUdf(
    name: string,
    extra?: types.UdfOrProcedureExtraParameter,
  ): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async listProcedure(): Promise<types.NamedStaff[]> {
    throw new Error("Method not implemented.");
  }

  async showProcedure(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<types.ProcedureDetail> {
    throw new Error("Method not implemented.");
  }

  async dropProcedure(
    name: string,
    arg?: types.UdfOrProcedureExtraParameter,
  ): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async tableExists(table: string): Promise<boolean> {
    throw new Error("Method not implemented.");
  }

  async listCreatedTypes(): Promise<types.CreatedTypeDef[]> {
    throw new Error("Method not implemented.");
  }

  async dropCreatedType(name: string): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async exportTableToString(
    tableName: string,
    kind: types.ExportKind,
  ): Promise<string> {
    throw new Error("Method not implemented.");
  }

  async listSequence(): Promise<types.SequenceDetail[]> {
    throw new Error("Method not implemented.");
  }

  async renameSequence(oldName: string, newName: string): Promise<void> {
    throw new Error("Method not implemented.");
  }

  async dropSequence(name: string, cascade: boolean): Promise<void> {
    throw new Error("Method not implemented.");
  }

  listSqlCurrentValues(): string[] {
    throw new Error("Method not implemented.");
  }

  async listSchema(): Promise<string[]> {
    const sql = "select schemaname from syscat.schemata";
    let r = await this.conn.query(sql);
    return r.map((e) => e.SCHEMANAME.trim());
  }

  async openSchema(name: string): Promise<SchemaTrait> {
    const config = { ...this.config };
    config.schema = name;
    return DB2Server.connect(config);
  }

  async createSchema(name: string): Promise<void> {
    const sql = `CREATE SCHEMA ${name}`;
    await this.conn.query(sql);
  }

  async dropSchema(name: string): Promise<void> {
    const sql = `DROP SCHEMA ${name}`;
    await this.conn.query(sql);
  }

  async getAutoCommit(queryId: string): Promise<boolean> {
    throw new Error("getAutoCommit method not implemented.");
  }

  async setAutoCommit(queryId: string): Promise<void> {
    throw new Error("setAutoCommit method not implemented.");
  }

  async commit(queryId: string): Promise<void> {
    throw new Error("commit method not implemented.");
  }

  async rollback(queryId: string): Promise<void> {
    throw new Error("rollback method not implemented.");
  }

  async querySql(
    queryId: string,
    sql: string,
    emit: () => void,
  ): Promise<void> {
    throw new Error("querySql method not implemented.");
  }

  async executeSql(queryId: string, sql: string): Promise<void> {
    throw new Error("executeSql method not implemented.");
  }

  async executeSqlNoOpenConn(sql: string): Promise<void> {
    throw new Error("executeSqlNoOpenConn method not implemented.");
  }

  async openConnection(id: string): Promise<void> {
    throw new Error("openConnection method not implemented.");
  }

  async closeConnection(id: string): Promise<void> {
    throw new Error("closeConnection method not implemented.");
  }

  async checkConn(): Promise<number> {
    const now = new Date();
    await this.conn.query(SELECT_1_SQL);
    return new Date().getTime() - now.getTime();
  }

  async showMetas(): Promise<types.MetaValue[]> {
    return [];
  }

  readOnly(): boolean {
    return this.config.read_only;
  }

  async exportQueryResultToFile(
    queryId: string,
    sql: string,
    kind: types.ExportKind,
    file: string,
  ): Promise<void> {
    throw new Error("exportQueryResultToFile method not implemented.");
  }
}

export { DB2Server };

const SELECT_1_SQL = "SELECT 1 FROM SYSIBM.SYSDUMMY1";
