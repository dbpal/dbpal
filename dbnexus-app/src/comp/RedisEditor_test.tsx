import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';
import { Position, languages } from 'monaco-editor';
import { RedisEditor } from './RedisEditor.tsx';
const root = ReactDOM.createRoot(document.getElementById('root'));

async function testAutoCompletion(
  _text: string,
  _pos: Position
): Promise<languages.CompletionList> {
  console.log('##################', _text, _pos);
  return {
    suggestions: [
      {
        label: 'get',
        kind: languages.CompletionItemKind.Keyword,
        insertText: 'get',
        range: undefined,
      },
      {
        label: 'set',
        kind: languages.CompletionItemKind.Keyword,
        insertText: 'get',
        range: undefined,
      },
    ],
  };
}

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
    <RedisEditor autoCompletion={testAutoCompletion} />
    <React.StrictMode></React.StrictMode>
  </NotificationContext.Provider>
);
