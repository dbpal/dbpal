import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MongoTable, MongoTableAdd } from './MongoDB.tsx';
import { parseMongoBson } from './MongoBsonCompiler.ts';
import {
  testBsonDoc,
  testBsonDocTableList,
  testDocsAndCount,
  testExampleMongoTableList,
} from './exampleTest.ts';
import { BSON, MaxKey } from 'bson';
import { voidTestFunction } from './helper.ts';
import { NotificationContext } from './Notification.tsx';
import { AlertColor, Button, IconButton } from '@mui/material';
import { MongoDataViewModel } from './common.tsx';

let x = parseMongoBson(`
{
  string: -1,
  b : 3
}

  `);

// console.log(x);
// console.log(/haha/i);
// console.log(eval('/haha/i'));

let doc = testBsonDoc();

root.render(
  <NotificationContext.Provider
    value={{
      showNotification: (
        severity: AlertColor,
        title: string,
        message: string
      ) => {
        console.log(`${severity} ${title} ${message}`);
        alert(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      {/* <MongoTableAdd addOP={voidTestFunction}></MongoTableAdd> */}
      {/* <MongoBsonEditor
        docPrev={doc}
        handleChange={() => {}}
        buttons={[]}
      ></MongoBsonEditor> */}

      <MongoTable
        viewModel={MongoDataViewModel.Object}
        reloadOPTableList={async (...args) => {
          try {
            voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testExampleMongoTableList();
        }}
        realodOPDocs={async (...args) => {
          return testDocsAndCount();
        }}
        modifyOP={voidTestFunction}
        loadOne={async () => {
          return testBsonDoc();
        }}
        deleteOP={voidTestFunction}
      ></MongoTable>
    </React.StrictMode>
  </NotificationContext.Provider>
);
