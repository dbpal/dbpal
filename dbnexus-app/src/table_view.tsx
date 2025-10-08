'use strict';
import ReactDOM from 'react-dom/client';
import React from 'react';
import {
  DataSourceKind,
  MongoTableHandler,
  RedisTableType,
  webviewWIndowInit,
} from './comp/helper.ts';
import { SqlTableView } from './comp/SqlTableView.tsx';
import {
  initMonacoLanguage,
  makeExtractTableSelectSort,
  makeExtractTableSelectWhere,
  makeInvalidCassandraTableCache,
  makePutValueInSql,
  makeShowTableFn,
  makeShowViewFn,
  makeTableDelete,
  makeTableUpdate,
} from './comp/Common.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { MongoTable } from './comp/MongoDB.tsx';
import { makeRedisViewTable } from './comp/LeftTable.tsx';
import { Typography } from '@mui/material';

webviewWIndowInit(false);
const params = new URLSearchParams(location.search);

const uuid = params.get('uuid');
const table = params.get('table');
const isTable = params.get('isTable') === 'true';
const kind = params.get('kind') as DataSourceKind;
const redisType = params.get('redisType') as RedisTableType;
console.log(uuid, table, isTable, kind);
let err;
await initMonacoLanguage(uuid, kind, (msg) => {
  err = msg;
});

let content;

switch (kind) {
  case DataSourceKind.Redis:
    content = makeRedisViewTable(uuid, table, redisType);
    break;
  case DataSourceKind.Mongo:
    content = (
      <MongoTable
        reloadOPTableList={MongoTableHandler.reloadTableList(uuid, table)}
        reloadOPDocs={MongoTableHandler.reloadDocs(uuid, table)}
        modifyOP={isTable ? MongoTableHandler.modifyOP(uuid, table) : undefined}
        loadOne={MongoTableHandler.loadOne(uuid, table)}
        deleteOP={isTable ? MongoTableHandler.deleteOP(uuid, table) : undefined}
      ></MongoTable>
    );
    break;

  default:
    content = (
      <SqlTableView
        extractTableSelectSort={makeExtractTableSelectSort(uuid)}
        extractTableSelectWhere={makeExtractTableSelectWhere(uuid)}
        invalidCassandraTableViewCache={makeInvalidCassandraTableCache(
          uuid,
          table
        )}
        tableName={table}
        uuid={uuid}
        kind={kind}
        reloadOP={
          isTable ? makeShowTableFn(uuid, table) : makeShowViewFn(uuid, table)
        }
        isTable={isTable}
        updateOP={isTable ? makeTableUpdate(uuid, table) : undefined}
        deleteOP={isTable ? makeTableDelete(uuid, table) : undefined}
        put_value_in_sql={makePutValueInSql(uuid)}
      ></SqlTableView>
    );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      {err && <Typography color="error"> {err} </Typography>} {content}
    </SeparateWindow>
  </React.StrictMode>
);
