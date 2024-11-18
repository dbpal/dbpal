import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';

import { Position, languages } from 'monaco-editor';

import { initRedisLanguage, RedisQuery } from './RedisQuery.tsx';
import { testExampleMultiQueryResp } from './exampleTest.ts';
const root = ReactDOM.createRoot(document.getElementById('root'));

async function testAutoCompletion(
  _text: string,
  _pos: Position
): Promise<languages.CompletionList> {
  return {
    suggestions: [
      {
        label: 'get',
        kind: languages.CompletionItemKind.Keyword,
        insertText: '',
        range: undefined,
      },
      {
        label: 'set',
        kind: languages.CompletionItemKind.Keyword,
        insertText: '',
        range: undefined,
      },
    ],
  };
}

initRedisLanguage(testAutoCompletion);

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
      <RedisQuery
        executeOP={async (_text) => {
          return testExampleMultiQueryResp(3);
        }}
        validURL={async (_url: string) => {
          return true;
        }}
      ></RedisQuery>
    </React.StrictMode>
  </NotificationContext.Provider>
);
