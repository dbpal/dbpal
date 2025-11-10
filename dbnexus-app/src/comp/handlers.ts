import { invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import { DataSourceKind, MuiDataGridData } from "./helper";

function makeExtractTableSelectWhere(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_selection", {
      name: uuid,
      sql,
    });
  };
}

function makeExtractTableSelectSort(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_sort", {
      name: uuid,
      sql,
    });
  };
}

function makeShowTableFn(uuid: string, table: string) {
  return async (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number,
    extra,
  ) => {
    const where_null = where ? where : null;
    const orderBy_null = orderBy ? orderBy : null;
    return await invoke<MuiDataGridData>("show_table", {
      name: uuid,
      tableName: table,
      whereStr: where_null,
      orderBy: orderBy_null,
      page,
      pageSize,
      extra,
    });
  };
}

function makeShowViewFn(uuid: string, view: string) {
  return async (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number,
    extra,
  ) => {
    const where_null = where ? where : null;
    const orderBy_null = orderBy ? orderBy : null;
    return await invoke<MuiDataGridData>("show_view", {
      name: uuid,
      viewName: view,
      whereStr: where_null,
      orderBy: orderBy_null,
      page,
      pageSize,
      extra,
    });
  };
}

function makeTableUpdate(uuid: string, table: string) {
  return async (wheres, values) => {
    return await invoke<types.SqlExecuteRet>("table_update", {
      name: uuid,
      tableName: table,
      values,
      wheres,
    });
  };
}

function makeTableDelete(uuid: string, table: string) {
  return async wheres => {
    return await invoke<types.SqlExecuteRet>("table_delete", {
      name: uuid,
      tableName: table,
      wheres,
    });
  };
}

function makeInvalidCassandraTableCache(uuid: string, table: string) {
  return async () => {
    return invoke<void>("cassandra_invalid_table_or_view_caching", {
      name: uuid,
      tableName: table,
    });
  };
}

function makePutValueInSql(uuid: string) {
  return async (typ, value) => {
    return invoke<string>("put_value_in_sql", {
      name: uuid,
      typ,
      value: value ? value : null,
    });
  };
}

function showViewColumns(uuid: string, view: string) {
  return invoke<types.Field[]>("show_view_columns", {
    name: uuid,
    viewName: view,
  });
}

function listSchema(name: string) {
  return invoke<string[]>("list_schema", { name });
}

function getLocale(): Promise<string> {
  return invoke("get_locale", {});
}

function listUdf(uuid: string) {
  return invoke<types.NamedStaff[]>("list_udf", { name: uuid });
}

function makeUdfExtraArg(udf: types.NamedStaff, kind: DataSourceKind) {
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
function showUdf(uuid: string, udf: types.NamedStaff, kind: DataSourceKind) {
  let arg = makeUdfExtraArg(udf, kind);
  return invoke<types.DefAndMetas>("show_udf", {
    name: uuid,
    udf: udf.name,
    arg,
  });
}

function listCassandraAgg(uuid: string) {
  return invoke<types.NamedStaff[]>("cassandra_list_aggregate", { name: uuid });
}

function showCassandraAgg(uuid: string, agg: string, parameters: string[]) {
  return invoke<types.DefAndMetas>("cassandra_show_aggregate", {
    name: uuid,
    aggName: agg,
    parameters,
  });
}

function showTableColumn(uuid: string, table: string) {
  return invoke<types.Field[]>("show_table_columns", {
    name: uuid,
    tableName: table,
  });
}

function makeTableInsertFn(uuid: string, table: string) {
  return async values => {
    return invoke<void>("table_insert", {
      name: uuid,
      tableName: table,
      values,
    });
  };
}

function listCreatedTypes(uuid: string) {
  return invoke<types.CreatedTypeDef[]>("list_created_types", { name: uuid });
}

function listEvent(uuid: string) {
  return invoke<types.SqlEvent[]>("list_event", { name: uuid });
}

function listProcedure(uuid: string) {
  return invoke<types.NamedStaff[]>("list_procedure", { name: uuid });
}

function listSequence(uuid: string) {
  return invoke<types.SequenceDetail[]>("list_sequence", { name: uuid });
}

async function callUnsupported<T>(x: () => Promise<T>, defaultValue = []) {
  try {
    return await x();
  } catch (e) {
    if (import.meta.env.DEV) {
      console.error(e);
    }
    return defaultValue as T;
  }
}

function listMaterializedView(uuid: string) {
  return invoke<types.ListTable[]>("list_materialized_view", { name: uuid });
}

function makeShowProcedureExtraArg(udf: types.NamedStaff, kind: DataSourceKind) {
  return;
  kind == DataSourceKind.Cockroach || kind == DataSourceKind.Postgres
    ? { extra: "postgres", parameters: udf.parameters }
    : null;
}

function showProcedure(uuid: string, udf: types.NamedStaff, kind: DataSourceKind) {
  const arg = makeShowProcedureExtraArg(udf, kind);
  return invoke<types.ProcedureDetail>("show_procedure", {
    name: uuid,
    procedure: udf.name,
    arg,
  });
}

export {
  makeExtractTableSelectWhere,
  makeExtractTableSelectSort,
  makeShowTableFn,
  makeShowViewFn,
  makeTableUpdate,
  makeTableDelete,
  makeInvalidCassandraTableCache,
  makePutValueInSql,
  showTableColumn,
  makeTableInsertFn,
  showViewColumns,
  listSchema,
  getLocale,
  listUdf,
  showUdf,
  listCassandraAgg,
  showCassandraAgg,
  listCreatedTypes,
  listProcedure,
  listEvent,
  listSequence,
  callUnsupported,
  listMaterializedView,
  makeUdfExtraArg,
  makeShowProcedureExtraArg,
  showProcedure,
};
