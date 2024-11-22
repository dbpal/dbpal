import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MongoBsonEditor, MongoTableList } from './MongoDB.tsx';
import {
  parseMongoBson,
  formatMongoBsonStr,
  bsonValue2MongoBson,
  MongoBsonObject,
} from './MongoBsonCompiler.ts';
import {
  testBsonDoc,
  testBsonDocTableList,
  testExampleMongoTableList,
} from './exampleTest.ts';
import { BSON, MaxKey } from 'bson';
import { voidTestFunction } from './helper.ts';
import { NotificationContext } from './Notification.tsx';
import { AlertColor } from '@mui/material';

// let x = parseMongoBson(`
// {
//   a : /haha/1,
// }
//   `);

// console.log(x);
// console.log(/haha/i);
// console.log(eval('/haha/i'));

let doc = BSON.serialize(testBsonDoc());

let value = bsonValue2MongoBson(doc);
let value2 = value.format(2);

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
      <MongoBsonEditor
        doc={{ aa: 100 }}
        maxHeight={500}
        handleChange={() => {}}
      ></MongoBsonEditor>
      <MongoTableList
        reloadOP={async (...args) => {
          try {
            voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testExampleMongoTableList();
        }}
        modifyOP={voidTestFunction}
        loadOne={async () => {
          return testBsonDoc();
        }}
      ></MongoTableList>
    </React.StrictMode>
  </NotificationContext.Provider>
);
