import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';
import { AlertColor } from '@mui/material';
import React from 'react';
import {
  RedisEditHexOrText,
  RedisHashTable,
  RedisListTable,
  RedisModifyKeyName,
  RedisSetTable,
  RedisStringTable,
  RedisZsetTable,
} from './Redis.tsx';
import { voidTestFunction } from './helper';
import {
  testRedisHash,
  testRedisList,
  testRedisSet,
  testRedisZset,
} from './exampleTest.ts';

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
  <NotificationContext.Provider
    value={{
      showNotification: (
        severity: AlertColor,
        title: string,
        message: string
      ) => {
        console.log(`${severity} ${title} ${message}`);
        // alert(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      {/* // start  */}

      {/* <RedisModifyKeyName
        modifyOP={voidTestFunction}
        name='string_key'
        handleClose={() => {}}
      ></RedisModifyKeyName>

      <RedisEditHexOrText
        oldValue='aaaaaabb'
        handleValue={(e) => {
          console.log(e);
        }}
      ></RedisEditHexOrText>
      <hr></hr> */}

      {/* <RedisListTable
        //@ts-ignore
        reloadOP={async (page, pageSize, asc) => {
          console.log('request data:', page, pageSize, asc);
          try {
            await voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testRedisList(pageSize);
        }}
        modifyOP={voidTestFunction}
        lpushOP={voidTestFunction}
        rpushOP={voidTestFunction}
        delOP={voidTestFunction}
      ></RedisListTable> */}

      {/* <RedisSetTable
        reloadOP={async (_: string) => {
          try {
            await voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testRedisSet(10);
        }}
        modifyOP={voidTestFunction}
        addOP={voidTestFunction}
        delOP={async () => {}}
      ></RedisSetTable> */}

      <RedisHashTable
        reloadOP={async (_keyMatch) => {
          return testRedisHash();
        }}
        addOP={voidTestFunction}
        modifyOP={voidTestFunction}
        delOP={voidTestFunction}
      ></RedisHashTable>

      {/* <RedisZsetTable
        reloadOP={async (_keyMatch, _min, _max) => {
          return testRedisZset();
        }}
        addOP={voidTestFunction}
        modifyOP={voidTestFunction}
        delOP={voidTestFunction}
      ></RedisZsetTable> */}

      {/* <RedisStringTable
        content="aaaaaabbb"
        modifyOP={voidTestFunction}
      ></RedisStringTable> */}

      {/* // end  */}
    </React.StrictMode>
  </NotificationContext.Provider>
);
