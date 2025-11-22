"use strict";
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
import {
  makeExtractTableSelectSort,
  makeExtractTableSelectWhere,
  makeInvalidCassandraTableCache,
  makePutValueInSql,
  makeShowTableFn,
  makeShowViewFn,
  makeTableDelete,
  makeTableInsertFn,
  makeTableUpdate,
} from "./comp/handlers.ts";

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
      <SqlTableView
        extractTableSelectSort={makeExtractTableSelectSort(uuid)}
        extractTableSelectWhere={makeExtractTableSelectWhere(uuid)}
        invalidCassandraTableViewCache={makeInvalidCassandraTableCache(uuid, table)}
        tableName={table}
        uuid={uuid}
        kind={kind}
        reloadOP={isTable ? makeShowTableFn(uuid, table) : makeShowViewFn(uuid, table)}
        addOP={isTable ? makeTableInsertFn(uuid, table) : undefined}
        isTable={isTable}
        updateOP={isTable ? makeTableUpdate(uuid, table) : undefined}
        deleteOP={isTable ? makeTableDelete(uuid, table) : undefined}
        put_value_in_sql={makePutValueInSql(uuid)}
      />
    );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>{content}</SeparateWindow>
  </React.StrictMode>,
);
