import { Channel } from "@tauri-apps/api/core";
import { ViewKind } from "./helper";
import {
  commands,
  CreateTableAndIndexSql,
  MuiDataGridData,
  QueryEvent,
  ShowTableParameter,
} from "./bindings";
import { Dispatch, SetStateAction } from "react";

function formatCreateTableAndIndexSql(sql: CreateTableAndIndexSql) {
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
  const indexes = sql.indexes.map((x) => newLine(x));
  ret += indexes.join("");
  return ret;
}

function listView(uuid: string, viewKind: ViewKind) {
  switch (viewKind) {
    case ViewKind.View:
      return commands.listView(uuid);
    case ViewKind.MaterializedView:
      return commands.listMaterializedView(uuid);
  }
}

async function cancelTask(taskId: string): Promise<void> {
  try {
    await commands.cancelTask(taskId);
  } catch (e) {
    console.error(e);
  }
}

interface ShowTableCallBack {
  handleResult: (_: MuiDataGridData) => void;
  okCallBack?: () => void;
  setLoading: Dispatch<SetStateAction<boolean>>;
}

async function showTable(
  props: {
    uuid: string;
    para?: ShowTableParameter;
    name: string;
    isView?: boolean;
    showNotification;
    setStatus: (_: string) => void;
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
    setStatus,
  } = props;
  isView = !!isView;
  if (!para) {
    para = {};
  }
  try {
    setLoading(true);
    let resp: MuiDataGridData = null;
    if (!para.where_str) {
      para.where_str = null;
    }
    if (!para.order_by) {
      para.order_by = null;
    }
    const emit = new Channel<QueryEvent>((event) => {
      if (import.meta.env.DEV) {
        console.log("show table event:", event);
      }
      switch (event.kind) {
        case "structure":
          if (event.resp.data.length > 0) {
            setStatus(`server send ${event.resp.data.length} records`);
          } else {
            setStatus(`server send me column definition.`);
          }
          resp = event.resp;
          handleResult(resp);
          break;
        case "data": {
          setStatus(`server send ${event.data.length} records`);
          let cop = { ...resp };
          cop.data = [
            ...cop.data,
            ...event.data.map((d) => {
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
          setStatus(`${event.status}`);
          break;
        case "end":
          setStatus("");
          break;
        default:
          console.error("server send wrong kind of data:", event);
      }
    });
    await commands.showTable(uuid, isView, name, para, emit);
    if (okCallBack) {
      okCallBack();
    }
  } catch (e) {
    showNotification({ severity: "error", title: "fetch data failed", message: e });
  } finally {
    setLoading(false);
    setStatus("");
  }
}

export { cancelTask, formatCreateTableAndIndexSql, listView, showTable };

export type { ShowTableCallBack };
