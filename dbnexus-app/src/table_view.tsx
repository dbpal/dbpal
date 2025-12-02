import ReactDOM from "react-dom/client";
import React, { ReactNode } from "react";
import {
  DataSourceKind,
  MongoTableHandler,
  NewWindowTableParameter,
  SomeTempStore,
} from "./comp/helper.ts";
import { SqlTableView } from "./comp/SqlTableView.tsx";

import { SeparateWindow } from "./comp/MainPage.tsx";
import { MongoTable } from "./comp/MongoDB.tsx";
import { makeRedisViewTable } from "./comp/LeftTable.tsx";

const { uuid, kind, table, redisTableType, isTable } =
  SomeTempStore.get<NewWindowTableParameter>();

let content: ReactNode;

switch (kind) {
  case DataSourceKind.Redis:
    content = makeRedisViewTable(uuid, table, redisTableType);
    break;
  case DataSourceKind.Mongo:
    content = (
      <MongoTable
        reloadOPTableList={MongoTableHandler.reloadTableList(uuid, table)}
        reloadOPDocs={MongoTableHandler.reloadDocs(uuid, table)}
        modifyOP={isTable ? MongoTableHandler.modifyOP(uuid, table) : undefined}
        loadOne={MongoTableHandler.loadOne(uuid, table)}
        deleteOP={isTable ? MongoTableHandler.deleteOP(uuid, table) : undefined}
      />
    );
    break;

  default:
    content = (
      <SqlTableView tableName={table} uuid={uuid} kind={kind} isTable={isTable} />
    );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>{content}</SeparateWindow>
  </React.StrictMode>,
);
