import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MongoBson } from './MongoDB.tsx';
import {
  parseMongoBson,
  formatMongoBsonStr,
  bsonValue2MongoBson,
  MongoBsonObject,
} from './MongoBsonCompiler.ts';
import { testBsonDoc } from './exampleTest.ts';
import { BSON } from 'bson';

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

console.log(
  '@@@@@@@@@@@@@@@@@',
  (parseMongoBson(value2) as MongoBsonObject).compile()
);

root.render(
  <React.StrictMode>
    {/* <QueryResp data={testExampleQueryResp()}></QueryResp> */}
    <MongoBson content={value2}></MongoBson>
  </React.StrictMode>
);
