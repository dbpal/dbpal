import { Editor, Monaco } from '@monaco-editor/react';
import { IconButton, Stack, Tooltip } from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { editor, Range } from 'monaco-editor';

import * as loglevel from 'loglevel';
import { useState } from 'react';
import { initMongoQueryGlobals } from './MongoQueryBackend';

initMongoQueryGlobals();

function makeLog(level, ...msg: any[]): string {
  let encode = (e): string => {
    switch (typeof e) {
      case 'undefined':
        return 'undefined';
      case 'boolean':
        return e.toString();
      case 'number':
      case 'bigint':
        return e.toString();
      case 'string':
        return e;
      default:
        JSON.stringify(e);
    }
  };

  let msg2 = msg.flat();
  let str = msg2.map((e) => encode(e)).join(' ');
  return `${new Date().toLocaleString()}-${level}-${str}\n`;
}

function MongoQuery(props: {}) {
  let logger = loglevel.getLogger('1111111111111');

  logger.setLevel('trace');
  const [text, setText] = useState('');
  const logEditorOnmount = (
    editor: editor.IStandaloneCodeEditor,
    monaco: Monaco
  ) => {
    logger.methodFactory = (level) => {
      return (...args) => {
        let l = makeLog(level, args);
        console.log(l);
        // append to the editor.
        appendEditorContent(editor, l);
      };
    };
  };
  const jsCodeMount = (
    editor: editor.IStandaloneCodeEditor,
    monaco: Monaco
  ) => {
    editor.onDidChangeModelContent((e) => {
      let text = editor.getValue();
      setText(text);
    });
  };

  const handleExecute = () => {
    console.log('execute query:', text);
    eval(text);
  };

  return (
    <Stack>
      <Stack direction="row">
        <Tooltip title="execute">
          <IconButton
            onClick={() => {
              handleExecute();
            }}
            color="primary"
            aria-label="execute"
          >
            <PlayArrowIcon />
          </IconButton>
        </Tooltip>
      </Stack>
      <div style={{ minHeight: '300px', height: '500px' }}>
        <Editor
          value={text}
          onMount={jsCodeMount}
          language="javascript"
          theme={'vs-dark'}
        ></Editor>
      </div>
      <div style={{ minHeight: '400px', height: '500px' }}>
        <Editor
          language="log"
          theme={'vs-dark'}
          onMount={logEditorOnmount}
        ></Editor>
      </div>
    </Stack>
  );
}

function appendEditorContent(editor: editor.IStandaloneCodeEditor, text) {
  // 获取当前编辑器的模型
  const model = editor.getModel();
  if (model) {
    // 获取当前模型的最后一行和最后一列
    const lastLine = model.getLineCount();
    const lastColumn = model.getLineLength(lastLine) + 1;

    // 创建一个 Range 对象，表示要插入的位置
    const range = new Range(lastLine, lastColumn, lastLine, lastColumn);

    // 创建一个编辑操作
    const editOperation = {
      identifier: { major: 1, minor: 1 },
      range: range,
      text: text,
      forceMoveMarkers: true,
    };
    // 执行编辑操作

    editor.executeEdits('appendContent', [editOperation]);
    // TODO show last line
    editor.setPosition({ lineNumber: lastLine, column: 1 });
    // 滚动到最后一行
    editor.revealLine(lastLine);
  } else {
    editor.setValue(text);
  }
}

export { MongoQuery };
