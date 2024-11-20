import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MongoBson } from './MongoDB.tsx';
import { parseMongoBson, formatMongoBson } from './MongoBsonCompiler.ts';

let x = parseMongoBson(`
{
  a : /haha/1,
}
  `);

console.log(x);
console.log(/haha/i);
console.log(eval('/haha/i'));

root.render(
  <React.StrictMode>
    {/* <QueryResp data={testExampleQueryResp()}></QueryResp> */}
    <MongoBson
      content={`
      {
    "a": "100" as int,
    "a1": 100 as int,
    "a2": 100 as long,
    "b": "200" as long,
    "xxx": [
        1,
        2,
        3
    ] as binary
}
    `}
    ></MongoBson>
  </React.StrictMode>
);
