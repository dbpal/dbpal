import ReactDOM from "react-dom/client";
import { ReactNode } from "react";
import {
  DataSourceKind,
  NewWindowTableParameter,
  secondConfirm,
  SetCloseConfirmTextContext,
  SomeTempStore,
} from "./comp/helper.ts";
import { SqlTableView } from "./comp/SqlTableView.tsx";

import { SeparateWindow } from "./comp/MainPage.tsx";
import { MongoTable } from "./comp/MongoDB.tsx";
import { makeRedisViewTable } from "./comp/LeftTable.tsx";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

const { uuid, kind, table, redisTableType, isTable } =
  SomeTempStore.get<NewWindowTableParameter>();

let content: ReactNode;

switch (kind) {
  case DataSourceKind.Redis:
    content = makeRedisViewTable(uuid, table, redisTableType);
    break;
  case DataSourceKind.Mongo:
    content = <MongoTable uuid={uuid} table={table} isTable={isTable} />;
    break;
  default:
    content = (
      <SqlTableView tableName={table} uuid={uuid} kind={kind} isTable={isTable} />
    );
}

let closeMsg = "";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SetCloseConfirmTextContext.Provider
    value={m => {
      closeMsg = m;
    }}
  >
    <SeparateWindow>{content}</SeparateWindow>
  </SetCloseConfirmTextContext.Provider>,
);

const webview = WebviewWindow.getCurrent();

webview.onCloseRequested(async event => {
  if (closeMsg) {
    const confirm = await secondConfirm({
      title: "close the window",
      content: closeMsg,
      confirmText: "close",
    });
    if (!confirm) {
      event.preventDefault();
    }
  }
});
