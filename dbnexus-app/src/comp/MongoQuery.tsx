import { Editor, Monaco } from '@monaco-editor/react';
import {
  Alert,
  Box,
  Chip,
  IconButton,
  Modal,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { editor, Position, Range } from 'monaco-editor';
import FavoriteIcon from '@mui/icons-material/Favorite';
import * as loglevel from 'loglevel';
import { useContext, useEffect, useMemo, useRef, useState } from 'react';
import {
  initMongoQueryGlobals,
  MongoDBQuery,
  BsonExtraDataType,
} from './MongoQueryBackend';
import { generateRandomString } from './helper';
import {
  EditorBar,
  EscDownEvent,
  LoadStoreFileForQuery,
  modalStyle,
  SettingContext,
} from './Common';
import { NotificationContext } from './Notification';
import { LogLevelSelection } from './CommonComp';
import * as esprima from 'esprima';
import { PiFileSqlFill } from 'react-icons/pi';
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

function MongoQuery(props: {
  uuid: string;
  favoriteName?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  value?: string;
}) {
  const { uuid, favoriteName: oldFavoriteName, saveFavoriteSql, value } = props;
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);

  const [err, setErr] = useState(null);
  const [loggerEditor, setLoggerEditor] = useState(null);
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [loggerPos, setLoggerPos] = useState<Position>(null);

  const [logger, _setLogger] = useState(
    loglevel.getLogger(generateRandomString(10))
  );
  const [loggerlevel, setLoggerlevel] = useState('trace');
  const [showLoggerLevel, setShowLoggerLevel] = useState(false);
  const [db, _] = useState(new MongoDBQuery(uuid));
  const [bsonExtraDataType, _setbsonExtraDataTypes] = useState(
    new BsonExtraDataType()
  );

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
  const setting = useContext(SettingContext);
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
      return;
    }
    showNotification(
      'success',
      'execute ok,but somehow async function or timeout function ... etc may running in background'
    );
  };

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.editorFontSize,
      ...setting.editorOption,
    };
  }, [setting]);
  const editorDivDom = useRef(null);
  useEffect(() => {
    if (editorDivDom.current) {
      (editorDivDom.current as HTMLDivElement).addEventListener(
        'keydown',
        function (event) {
          if (event.ctrlKey && event.key.toLowerCase() === 'e') {
            event.preventDefault();
            handleExecute();
          }
        }
      );
    }
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
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

        <Tooltip title="logger level">
          <Chip
            label={loggerlevel}
            color="primary"
            size="small"
            onClick={() => {
              setShowLoggerLevel(true);
            }}
          />
        </Tooltip>

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
            <PiFileSqlFill style={{ fontSize: '1.6rem' }} />
          </IconButton>
        </Tooltip>

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
          height: '60vh',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          width: '100%',
        }}
        ref={editorDivDom}
      >
        <div style={{ height: '100px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            width="99.5%"
            onMount={jsCodeMount}
            language="javascript"
            options={options}
            value={value}
            theme={setting.editorTheme()}
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

      <Chip
        color="info"
        label=" logger"
        style={{ fontWeight: 'bold', fontSize: '1.2rem' }}
        size="small"
      />
      <div
        style={{
          minHeight: '200px',
          height: '500px',
          resize: 'vertical',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
      >
        <div style={{ height: '100px', flexGrow: 1, flexShrink: 1 }}>
          <Editor
            options={options}
            width="99.5%"
            language="log"
            theme={setting.editorTheme()}
            onMount={logEditorOnmount}
          ></Editor>
        </div>
        <EditorBar pos={loggerPos}></EditorBar>
      </div>
      {showLoggerLevel && (
        <Modal
          open={showLoggerLevel}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowLoggerLevel(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <LogLevelSelection
              level={loggerlevel}
              handleChange={(level) => {
                setLoggerlevel(level);
                setShowLoggerLevel(false);
              }}
            ></LogLevelSelection>
          </Box>
        </Modal>
      )}

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
          <Box sx={modalStyle} width="55%">
            <Stack spacing={1}>
              <TextField
                fullWidth
                label="name"
                value={favoriteName}
                onChange={(e) => {
                  setFavoriteName(e.target.value);
                }}
                size="small"
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
                  <FavoriteIcon> </FavoriteIcon>
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
