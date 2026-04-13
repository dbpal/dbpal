import { Channel, invoke } from "@tauri-apps/api/core";
import { MuiDataGridData, ViewKind } from "./helper";
import { message } from "@tauri-apps/plugin-dialog";
import { QueryEvent, unwrapCommand } from "./Common";
import {
  commands,
  CreateTableAndIndexSql,
  ListTable,
  ShowTableParameter,
} from "./bindings";

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
  let invokeName = "";
  switch (viewKind) {
    case ViewKind.View:
      invokeName = "list_view";
      break;
    case ViewKind.MaterializedView:
      invokeName = "list_materialized_view";
      break;
  }
  return invoke<ListTable[]>(invokeName, { name: uuid });
}

async function cancelTask(taskId: string): Promise<void> {
  let r = await commands.cancelTask(taskId);
  if (r.status == "error") {
    message("cancel task failed,err" + String(r.error), { kind: "error" });
    console.error(r.error);
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
    para?: ShowTableParameter;
    name: string;
    isView?: boolean;
    showNotification;
    handleExecuteTime?: (ms: number) => void;
    setStatus: (string) => void;
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
    await unwrapCommand(commands.showTable(uuid, isView, name, para, emit));
    if (okCallBack) {
      okCallBack();
    }
  } catch (e) {
    showNotification({ severity: "error", title: "fetch data failed", message: e });
  } finally {
    if (handleExecuteTime) {
      handleExecuteTime(new Date().getTime() - now.getTime());
    }
    setLoading(false);
    setStatus("");
  }
}

export { cancelTask, formatCreateTableAndIndexSql, listView, showTable };

export type { ShowTableCallBack };
