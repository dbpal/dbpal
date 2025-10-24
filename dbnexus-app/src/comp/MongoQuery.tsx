'use strict';
import { Editor, Monaco } from '@monaco-editor/react';
import {
  Box,
  Button,
  IconButton,
  Modal,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { editor, Position } from 'monaco-editor';
import { useContext, useMemo, useRef, useState } from 'react';
import { initMongoQueryGlobals, MongoDBQuery } from './MongoQueryBackend';
import EventNoteIcon from '@mui/icons-material/EventNote';
import {
  DataSourceKind,
  QueryLogHandler,
  SaveQueryLogConfigure,
  SettingContext,
} from './helper';
import { EscDownEvent, modalHandleClose, modalStyle } from './Common';
import { NotificationContext } from './Notification';
import { ModalHeader } from './CommonComp';
import * as esprima from 'esprima';
import { FaBookmark } from 'react-icons/fa';
import {
  EditorBar,
  LoadStoreFileForQuery,
  SimpleFlexEditor,
} from './CommonComp2';
import { QueryLog } from './QueryLog';
import InfoIcon from '@mui/icons-material/Info';

import intervalCleanTest from '../../tests/interval_clean_test.js?raw';
import { BsonExtraDataType } from './MongoInterface';

initMongoQueryGlobals();

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
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [executeTimes, setExecuteTimes] = useState(0);
  const setting = useContext(SettingContext);
  const [db] = useState(new MongoDBQuery(uuid));
  const [cleanFns] = useState([]);
  const [bsonExtraDataType] = useState(new BsonExtraDataType());
  const { showNotification } = useContext(NotificationContext);
  const jsCodeMount = (ed: editor.IStandaloneCodeEditor, _monaco: Monaco) => {
    editor.current = ed;
    ed.focus();
    ed.onDidChangeCursorPosition(() => {
      setPos(ed.getPosition());
    });
  };
  const handleExecute = () => {
    const code = editor.current.getValue();
    if (!code.trim()) {
      showNotification('warning', "can't execute empty statement");
      return;
    }

    setErr(null);
    try {
      // capture some error from `esprima parser`.
      esprima.parseScript(code);
    } catch (e) {
      setErr(`${e.lineNumber}:${e.column} ${e.description}`);
    }
    setExecuteTimes((e) => e + 1);
    try {
      eval(code);
    } catch (e) {
      if (SaveQueryLogConfigure.get()) {
        new QueryLogHandler(uuid).save(code, false, showNotification);
      }
      setErr(e);
      console.log('mongo query error:', e);
      return;
    }
    if (SaveQueryLogConfigure.get()) {
      new QueryLogHandler(uuid).save(code, true);
    }
    showNotification(
      'success',
      'execute ok,but somehow async function or timeout function ... etc may running in background'
    );
  };

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.appearance.fontSize,
      fontWeight: setting.appearance.fontWeight,
      fontFamily: setting.appearance.fontFamily,
      tabSize: setting.editor.tabSize,
      ...setting.editor.editorOption,
    };
  }, [setting]);
  return (
    <Stack style={{ position: 'relative', height: '100%' }}>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <QueryLog
          showQueryLog={showQueryLog}
          setShowQueryLog={setShowQueryLog}
          uuid={uuid}
          kind={DataSourceKind.Mongo}
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
                showNotification('warning', "can't save empty statement.");
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
            editor.current.getAction('editor.action.formatDocument').run();
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

        <Button
          onClick={() => {
            for (const callback of cleanFns) {
              callback();
              try {
              } catch (e) {
                showNotification('error', 'clean error', e);
              }
            }
            cleanFns.length = 0; // clean function
          }}
          variant="outlined"
        >
          call clean function
        </Button>

        <Tooltip
          title=<CleanFunctionDoc width="55vw"></CleanFunctionDoc>
          slotProps={{
            tooltip: {
              style: { maxWidth: '55vw' },
            },
          }}
        >
          <InfoIcon> </InfoIcon>
        </Tooltip>
        <Tooltip
          title=<Stack width="55vw" style={{ maxHeight: '50vh' }}>
            <Typography color="info ">
              we are still using `console` to debug.
            </Typography>
            <Typography variant="h6">
              open dev tools:Keyboard Shortcuts:
            </Typography>
            <pre>
              Windows and Linux: Press Ctrl + Shift + i. macOS: Press Command +
              Option + i.
            </pre>
          </Stack>
          slotProps={{
            tooltip: {
              style: { maxWidth: '55vw' },
            },
          }}
        >
          <span>debug???</span>
        </Tooltip>
      </Stack>

      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          flex: 1,
        }}
      >
        <div style={{ height: '100px', flex: 1 }}>
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
      {showSetFavoriteName && (
        <Modal
          open={showSetFavoriteName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFavoriteName(false);
          })}
        >
          <Box sx={modalStyle} width="45vw">
            <ModalHeader
              title="Set Favorite Name"
              handleClose={() => {
                setShowFavoriteName(false);
              }}
            ></ModalHeader>
            <Stack>
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
                      showNotification('warning', "name can't be empty");
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
      {db && <span></span>}
      {bsonExtraDataType && <span></span>}
      {cleanFns && <span></span>}
    </Stack>
  );
}

function CleanFunctionDoc(props: { width: string }) {
  const { width } = props;
  return (
    <Stack width={width} height="40vh">
      <SimpleFlexEditor
        language="javascript"
        value={intervalCleanTest}
        readOnly
      />
    </Stack>
  );
}

export { MongoQuery };
