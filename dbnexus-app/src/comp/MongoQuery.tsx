import { Editor, Monaco } from '@monaco-editor/react';
import { Alert, Divider, IconButton, Stack, Tooltip } from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { editor, Position, Range } from 'monaco-editor';

import * as loglevel from 'loglevel';
import { useContext, useEffect, useRef, useState } from 'react';
import { initMongoQueryGlobals, MongoDBQuery } from './MongoQueryBackend';
import { generateRandomString, noUse } from './helper';
import { EditorBar, LoadStoreFileForQuery, SettingContext } from './Common';
import { NotificationContext } from './Notification';
import { LogLevelSelection } from './CommonComp';
import * as esprima from 'esprima';
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
        return JSON.stringify(e);
    }
  };
  let msg2 = msg.flat();
  let str = msg2.map((e) => encode(e)).join(' ');
  return `${new Date().toLocaleString()}-${level} ${str}\n`;
}

const fileFilter = { filters: [{ name: 'js', extensions: ['js'] }] };

function MongoQuery(props: { uuid: string }) {
  const { uuid } = props;

  const [err, setErr] = useState(null);
  const [loggerEditor, setLoggerEditor] = useState(null);
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [loggerPos, setLoggerPos] = useState<Position>(null);

  const [logger, _setLogger] = useState(
    loglevel.getLogger(generateRandomString(10))
  );
  const [loggerlevel, setLoggerlevel] = useState('trace');

  const db = new MongoDBQuery(uuid);
  noUse(db);
  useEffect(() => {
    logger.setLevel(loggerlevel as loglevel.LogLevelDesc);
  }, [loggerlevel]);

  const logEditorOnmount = (
    editor: editor.IStandaloneCodeEditor,
    _monaco: Monaco
  ) => {
    setLoggerEditor(editor);
    logger.methodFactory = (level) => {
      return (...args) => {
        let l = makeLog(level, args);
        modifyEditorContent(editor, l);
      };
    };
    // we need set up level when `methodFactory` is changed
    // otherwise logger will not working why
    // looks a bug to me.
    logger.setLevel('trace');
    editor.onDidChangeCursorPosition(() => {
      setLoggerPos(editor.getPosition());
    });
  };
  const [file, setFile] = useState('');

  const { showNotification } = useContext(NotificationContext);
  const jsCodeMount = (ed: editor.IStandaloneCodeEditor, _monaco: Monaco) => {
    editor.current = ed;
    ed.onDidChangeCursorPosition(() => {
      setPos(ed.getPosition());
    });
  };

  const handleExecute = () => {
    let text = editor.current.getValue();
    if (!text.trim()) {
      showNotification('error', "can't execute empty statement");
      return;
    }
    modifyEditorContent(loggerEditor);
    setErr(null);

    try {
      // capture some error from `esprima parser`.
      esprima.parseScript(text);
    } catch (e) {
      setErr(`${e.lineNumber}:${e.column} ${e.description}`);
      return;
    }

    try {
      eval(text);
    } catch (e) {
      setErr(e);
      console.log('mongo query error:', e);
    }
  };
  let theme = useContext(SettingContext).theme == 'dark' ? 'vs-dark' : 'light';

  return (
    <Stack spacing={1}>
      <Stack direction="row" sx={{ alignItems: 'flex-end' }} spacing={1}>
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
        <LogLevelSelection
          handleChange={(e) => {
            setLoggerlevel(e);
          }}
          level={loggerlevel}
        ></LogLevelSelection>
        <LoadStoreFileForQuery
          openOpt={{ ...fileFilter }}
          saveOpt={{ canCreateDirectories: true, ...fileFilter }}
          getQuery={() => {
            return editor.current.getValue();
          }}
          file={file}
          handleChange={(f, text) => {
            setFile(f);
            editor.current.setValue(text);
          }}
        ></LoadStoreFileForQuery>
      </Stack>
      <div
        style={{
          minHeight: '200px',
          height: '500px',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'auto',
        }}
      >
        <div style={{ height: '200px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            onMount={jsCodeMount}
            language="javascript"
            options={{
              fontFamily: 'Fira Code',
              fontSize: useContext(SettingContext).editorFontSize,
            }}
            theme={theme}
          ></Editor>
        </div>

        <EditorBar
          left={
            err && (
              <Alert color="error" severity="error">
                {err.toString()}
              </Alert>
            )
          }
          pos={pos}
        ></EditorBar>
      </div>

      <Divider></Divider>
      <div
        style={{
          minHeight: '200px',
          height: '500px',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'auto',
        }}
      >
        <div style={{ height: '200px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            options={{
              fontSize: useContext(SettingContext).editorFontSize,
              wordWrap: 'on',
            }}
            language="log"
            theme={theme}
            onMount={logEditorOnmount}
          ></Editor>
        </div>
        <EditorBar pos={loggerPos}></EditorBar>
      </div>
      <div style={{ minHeight: '400px' }}></div>
    </Stack>
  );
}
/*
  
*/
function modifyEditorContent(
  editor: editor.IStandaloneCodeEditor,
  // means clean text
  text = undefined
) {
  if (!text) {
    editor.setValue('');
    return;
  }
  const model = editor.getModel();
  if (model) {
    const lastLine = model.getLineCount();
    const lastColumn = model.getLineLength(lastLine) + 1;

    const range = new Range(lastLine, lastColumn, lastLine, lastColumn);

    const editOperation = {
      identifier: { major: 1, minor: 1 },
      range: range,
      text: text,
      forceMoveMarkers: true,
    } as editor.IIdentifiedSingleEditOperation;

    editor.executeEdits('appendContent', [editOperation]);
    // 滚动到最后一行
    editor.setPosition({ lineNumber: lastLine + 1, column: 1 });
    editor.revealLine(lastLine + 1);
  } else {
    editor.setValue(text);
  }
}

export { MongoQuery };
