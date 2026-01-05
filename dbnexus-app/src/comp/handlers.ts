import { Channel, invoke } from "@tauri-apps/api/core";
import types from "./ApiStruct";
import {
  DataSourceKind,
  ErrAndLoc,
  MuiDataGridData,
  SetStatusFn,
  TableType,
  ViewKind,
} from "./helper";
import { message } from "@tauri-apps/plugin-dialog";
import { QueryEvent } from "./Common";
import { RefObject } from "react";

async function tryGetMaxID(uuid: string, tableOrView: string) {
  try {
    const r = await invoke<string>("try_get_max_id", {
      name: uuid,
      table: tableOrView,
    });
    return r;
  } catch (e) {
    console.error("get max id failed", e);
    return null;
  }
}

function makeTableUpdate(uuid: string, table: string) {
  return async (wheres, values) => {
    return await invoke<string>("table_update_sql", {
      name: uuid,
      tableName: table,
      values,
      wheres,
    });
  };
}

function makeTableDelete(uuid: string, table: string) {
  return async wheres => {
    return await invoke<string>("table_delete_sql", {
      name: uuid,
      tableName: table,
      wheres,
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

function makeUdfExtraArg(
  udf: types.NamedStaff,
  kind: DataSourceKind,
): types.UdfOrProcedureExtraParameter {
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
  const arg = makeUdfExtraArg(udf, kind);
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

async function callErrorable<T>(x: () => Promise<T>, defaultValue: any = []) {
  try {
    return await x();
  } catch (e) {
    console.log(String(e));
    let r = "reason";

    if (
      !(
        typeof e == "object" &&
        r in e &&
        (e[r] as string).includes("don't support this feature")
      )
    ) {
      console.error(e);
    }
    return defaultValue as T;
  }
}

function listMaterializedView(uuid: string) {
  return invoke<types.ListTable[]>("list_materialized_view", { name: uuid });
}

function makeShowProcedureExtraArg(udf: types.NamedStaff, kind: DataSourceKind) {
  return kind == DataSourceKind.Cockroach || kind == DataSourceKind.Postgres
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

function showIndexes(uuid: string, table: string) {
  return invoke<types.Index[]>("show_indexes", {
    name: uuid,
    tableName: table,
  });
}

function showTrigger(uuid: string, table: string) {
  return invoke<types.TriggerDetail[]>("show_trigger", {
    name: uuid,
    tableName: table,
  });
}

function createTableAndIndexSql(uuid: string, table: string) {
  return invoke<types.CreateTableAndIndexSql>("show_create_table_sql", {
    name: uuid,
    tableName: table,
  });
}

function createViewSql(uuid: string, view: string, viewKind: ViewKind) {
  return invoke<string>("create_view_sql", {
    name: uuid,
    viewName: view,
    materialized: viewKind == ViewKind.MaterializedView,
  });
}
function formatCreateTableAndIndexSql(sql: types.CreateTableAndIndexSql) {
  function newLine(ret: string): string {
    if (!ret.endsWith(";")) {
      ret += ";\n";
    }
    if (!ret.endsWith("\n")) {
      ret += "\n";
    }
    return ret;
  }

  let ret = newLine(sql.create_table);
  const indexes = sql.indexes.map(x => newLine(x));
  ret += indexes.join("");
  return ret;
}

function listTable(uuid: string, typ: TableType) {
  let invokeName: string;
  switch (typ) {
    case TableType.Normal:
      invokeName = "list_table";
      break;
    case TableType.MongoTimeSerial:
      invokeName = "mongo_list_time_serial_collection";
      break;
  }
  return invoke<types.ListTable[]>(invokeName, { name: uuid });
}

function listView(uuid: string, viewKind: ViewKind) {
  let invokeName = "";
  switch (viewKind) {
    case ViewKind.View:
      invokeName = "list_view";
      break;
    case ViewKind.MaterializedView:
      invokeName = "list_materialized_view";
      break;
  }
  return invoke<types.ListTable[]>(invokeName, { name: uuid });
}

async function cancelTask(taskId: string, showNotification): Promise<void> {
  try {
    return await invoke<void>("cancel_task", { taskId });
    showNotification("success", "cancel task ok");
  } catch (e) {
    message("cancel task failed,err" + String(e), { kind: "error" });
    console.error(e);
  }
}

async function checkSql(uuid: string, sql: string) {
  try {
    return await invoke<ErrAndLoc | null>("check_sql", {
      name: uuid,
      sql,
    });
  } catch (e) {
    console.error("this check sql function should not throw error,but", e);
    return null;
  }
}

interface ShowTableCallBack {
  handleResult: (_: MuiDataGridData) => void;
  okCallBack?: () => void;
  setLoading;
}
async function showTable(
  props: {
    uuid: string;
    para?: types.ShowTableParameter;
    name: string;
    isView?: boolean;
    showNotification;
    handleExecuteTime?: (ms: number) => void;
    setStatus: RefObject<SetStatusFn>;
  } & ShowTableCallBack,
) {
  let {
    uuid,
    para,
    name,
    isView,
    okCallBack,
    showNotification,
    handleResult,
    setLoading,
    handleExecuteTime,
    setStatus,
  } = props;
  isView = !!isView;
  if (!para) {
    para = {};
  }
  let now = new Date();
  try {
    setLoading(true);
    let resp: MuiDataGridData = null;
    if (!para.where_str) {
      para.where_str = null;
    }
    if (!para.order_by) {
      para.order_by = null;
    }
    const emit = new Channel<QueryEvent>(event => {
      if (import.meta.env.DEV) {
        console.log("show table event:", event);
      }
      switch (event.kind) {
        case "structure":
          if (event.resp.data.length > 0) {
            setStatus.current(`server send ${event.resp.data.length} records`);
          } else {
            setStatus.current(`server send me column definition.`);
          }
          resp = event.resp;
          handleResult(resp);
          break;
        case "data": {
          setStatus.current(`server send ${event.data.length} records`);
          let cop = { ...resp };
          cop.data = [
            ...cop.data,
            ...event.data.map(d => {
              let obj = {};
              d.forEach((e, index) => {
                obj[cop.fields[index].field] = e;
              });
              return obj;
            }),
          ];
          handleResult(cop);
          resp = cop;
          break;
        }
        case "status":
          setStatus.current(`${event.status}`);
          break;
        case "end":
          setStatus.current("");
          break;
        default:
          console.error("server send wrong kind of data:", event);
      }
    });
    await invoke("show_table", { name: uuid, tableName: name, para, isView, emit });
    if (okCallBack) {
      okCallBack();
    }
  } catch (e) {
    showNotification("error", "fetch data failed", e);
  } finally {
    if (handleExecuteTime) {
      //@ts-ignore
      handleExecuteTime(new Date() - now);
    }
    setLoading(false);
    setStatus.current("");
  }
}

export {
  makeTableUpdate,
  makeTableDelete,
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
  callErrorable,
  listMaterializedView,
  makeUdfExtraArg,
  makeShowProcedureExtraArg,
  showProcedure,
  showIndexes,
  showTrigger,
  createTableAndIndexSql,
  createViewSql,
  formatCreateTableAndIndexSql,
  listTable,
  listView,
  tryGetMaxID,
  cancelTask,
  checkSql,
  showTable,
};

export type { ShowTableCallBack };
