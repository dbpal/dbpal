import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));
import { MultiQueryResp } from './QueryResp.tsx';
import { testMultiQueryResp, testQueryResp } from './exampleTest.ts';

root.render(
  <React.StrictMode>
    {/* <QueryResp data={testQueryResp()}></QueryResp> */}
    <MultiQueryResp data={testMultiQueryResp()}></MultiQueryResp>
  </React.StrictMode>
);
