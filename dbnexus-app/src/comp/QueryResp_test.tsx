import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MultiQueryResp } from './QueryResp.tsx';
import {
  testExampleMultiQueryResp,
  testExampleQueryResp,
} from './exampleTest.ts';

root.render(
  <React.StrictMode>
    {/* <QueryResp data={testExampleQueryResp()}></QueryResp> */}
    <MultiQueryResp data={testExampleMultiQueryResp()}></MultiQueryResp>
  </React.StrictMode>
);
