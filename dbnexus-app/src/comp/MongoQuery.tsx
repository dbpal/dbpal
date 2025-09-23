import { Editor, Monaco } from '@monaco-editor/react';
import {
  Box,
  Chip,
  IconButton,
  Modal,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { editor, Position, Range } from 'monaco-editor';
import * as loglevel from 'loglevel';
import { useContext, useEffect, useMemo, useRef, useState } from 'react';
import {
  initMongoQueryGlobals,
  MongoDBQuery,
  BsonExtraDataType,
} from './MongoQueryBackend';
import EventNoteIcon from '@mui/icons-material/EventNote';
import {
  DataSourceKind,
  generateRandomString,
  QueryLogHandler,
  saveAndSetSize,
  SettingContext,
} from './helper';
import { EscDownEvent, modalStyle } from './Common';
import { NotificationContext } from './Notification';
import { LogLevelSelection, ModalHeader } from './CommonComp';
import * as esprima from 'esprima';
import { FaBookmark } from 'react-icons/fa';
import { EditorBar, LoadStoreFileForQuery } from './CommonComp2';
import { QueryLog, QueryLogKind } from './QueryLog';
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

function MongoQuery(props: {
  uuid: string;
  favoriteName?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  value?: string;
}) {
  const { uuid, favoriteName: oldFavoriteName, saveFavoriteSql, value } = props;
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [err, setErr] = useState(null);
  const [loggerEditor, setLoggerEditor] = useState(null);
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [loggerPos, setLoggerPos] = useState<Position>(null);
  const [executeTimes, setExecuteTimes] = useState(0);
  const [logger, _setLogger] = useState(
    loglevel.getLogger(generateRandomString(10))
  );
  const [loggerLevel, setLoggerLevel] = useState('trace');
  const [db, _] = useState(new MongoDBQuery(uuid));
  const [bsonExtraDataType, _setBsonExtraDataTypes] = useState(
    new BsonExtraDataType()
  );

  useEffect(() => {
    logger.setLevel(loggerLevel as loglevel.LogLevelDesc);
  }, [loggerLevel]);

  const logEditorOnMount = (
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
  const { showNotification } = useContext(NotificationContext);
  const jsCodeMount = (ed: editor.IStandaloneCodeEditor, _monaco: Monaco) => {
    editor.current = ed;
    ed.onDidChangeCursorPosition(() => {
      setPos(ed.getPosition());
    });
  };
  const setting = useContext(SettingContext);
  const handleExecute = () => {
    let code = editor.current.getValue();
    if (!code.trim()) {
      showNotification('error', "can't execute empty statement");
      return;
    }
    modifyEditorContent(loggerEditor);
    setErr(null);

    try {
      // capture some error from `esprima parser`.
      esprima.parseScript(code);
    } catch (e) {
      setErr(`${e.lineNumber}:${e.column} ${e.description}`);
      return;
    }
    setExecuteTimes((e) => e + 1);
    try {
      eval(code);
    } catch (e) {
      new QueryLogHandler(uuid).save(code, false, showNotification);
      setErr(e);
      console.log('mongo query error:', e);
      return;
    }
    new QueryLogHandler(uuid).save(code, true);
    showNotification(
      'success',
      'execute ok,but somehow async function or timeout function ... etc may running in background'
    );
  };

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.fontSize,
      fontWeight: setting.fontWeight,
      fontFamily: setting.fontFamily,
      tabSize: setting.tabSize,
      ...setting.editorOption,
    };
  }, [setting]);
  const editorDom = useRef(null);
  const loggerDom = useRef(null);
  useEffect(() => {
    const x1 = saveAndSetSize(editorDom.current, 'mongo-query', 'height');
    const x2 = saveAndSetSize(loggerDom.current, 'mongo-logger', 'height');
    return () => {
      x1();
      x2();
    };
  }, []);

  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        sx={{ alignItems: 'center', position: 'relative' }}
        spacing={1}
      >
        <QueryLog
          showQueryLog={showQueryLog}
          setShowQueryLog={setShowQueryLog}
          uuid={uuid}
          language={'javascript'}
          kind={QueryLogKind.Mongo}
          executeTimes={executeTimes}
        ></QueryLog>

        <Tooltip title="execute">
          <IconButton
            onClick={() => {
              handleExecute();
            }}
            color="primary"
          >
            <PlayArrowIcon />
          </IconButton>
        </Tooltip>

        <LogLevelSelection
          level={loggerLevel}
          handleChange={(level) => {
            setLoggerLevel(level);
          }}
        ></LogLevelSelection>
        <Tooltip
          title={
            !favoriteName
              ? 'save sql to favorite.'
              : `save favorite sql as named '${favoriteName}'`
          }
        >
          <IconButton
            onClick={() => {
              const value = editor.current.getValue();
              if (!value) {
                showNotification('error', "can't save empty statement.");
                return;
              }
              if (favoriteName) {
                saveFavoriteSql(favoriteName, value);
              } else {
                setShowFavoriteName(true);
              }
            }}
            color="primary"
          >
            <FaBookmark style={{ fontSize: '1.3rem' }} />
          </IconButton>
        </Tooltip>

        <LoadStoreFileForQuery
          getQuery={() => {
            return editor.current.getValue();
          }}
          kind={DataSourceKind.Mongo}
          handleChange={(text) => {
            editor.current.setValue(text);
          }}
        ></LoadStoreFileForQuery>
        <Tooltip title="query log">
          <IconButton
            onClick={() => {
              setShowQueryLog((prev) => !prev);
            }}
            color="primary"
          >
            <EventNoteIcon></EventNoteIcon>
          </IconButton>
        </Tooltip>
      </Stack>

      <div
        style={{
          minHeight: '200px',
          height: '60vh',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          width: '100%',
        }}
        ref={editorDom}
      >
        <div style={{ height: '100px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            width="99%"
            height="99%"
            onMount={jsCodeMount}
            language="javascript"
            options={options}
            value={value}
            theme={setting.editorTheme()}
          ></Editor>
        </div>

        <EditorBar
          left={err && <Typography color="error">{err.toString()}</Typography>}
          pos={pos}
        ></EditorBar>
      </div>
      <Chip color="info" label=" logger" style={{ fontSize: '1.2rem' }} />
      <div
        style={{
          minHeight: '200px',
          height: '500px',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
        ref={loggerDom}
      >
        <div style={{ height: '100px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            options={options}
            width="99.5%"
            height="99%"
            language="log"
            theme={setting.editorTheme()}
            onMount={logEditorOnMount}
          ></Editor>
        </div>
        <EditorBar pos={loggerPos}></EditorBar>
      </div>
      {showSetFavoriteName && (
        <Modal
          open={showSetFavoriteName}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowFavoriteName(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader title="Set Favorite Name"></ModalHeader>
            <Stack spacing={1}>
              <TextField
                fullWidth
                label="name"
                value={favoriteName}
                onChange={(e) => {
                  setFavoriteName(e.target.value);
                }}
                required
              ></TextField>
              <Stack direction="row">
                <IconButton
                  color="primary"
                  onClick={() => {
                    if (!favoriteName) {
                      showNotification('error', "name can't be empty");
                      return;
                    }
                    saveFavoriteSql(favoriteName, editor.current.getValue());
                    setShowFavoriteName(false);
                  }}
                >
                  <FaBookmark> </FaBookmark>
                </IconButton>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
      {/* // we must use db etc ...  */}
      {db && <div></div>}
      {bsonExtraDataType && <div></div>}
    </Stack>
  );
}

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
