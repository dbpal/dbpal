import { Editor } from '@monaco-editor/react';
import SplitPane from 'react-split-pane';
import { MultiQueryResp } from './QueryResp';
import EventNoteIcon from '@mui/icons-material/EventNote';
import './style/react-split-pane.css';
import {
  Box,
  Chip,
  IconButton,
  Modal,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';

import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { FaExclamation } from 'react-icons/fa';
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import FastForwardIcon from '@mui/icons-material/FastForward';
import { editor, Position } from 'monaco-editor';
import {
  EditorBar,
  EscDownEvent,
  IQueryConn,
  LoadStoreFileForQuery,
  MakeIQueryConn,
  modalStyle,
  msgTimeout,
} from './Common.tsx';
import { BsFillLightbulbFill } from 'react-icons/bs';
import { FaBookmark } from 'react-icons/fa';
import { FaUpload } from 'react-icons/fa';
import { MdOutlineReply } from 'react-icons/md';
import { NotificationContext } from './Notification.tsx';
import { QueryLog } from './QueryLog.tsx';
import RefreshIcon from '@mui/icons-material/Refresh';
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  QueryLogHandler,
  SaveSizesToStore,
  secondConfirm,
  SetCloseConfirmTextContext,
  SettingContext,
  VisiableContext,
} from './helper.ts';
import ArrowCircleDownRoundedIcon from '@mui/icons-material/ArrowCircleDownRounded';
import { ExportView } from './Export.tsx';
import { invoke } from '@tauri-apps/api/core';
import { ExecuteTime, PossibleParseError } from './CommonComp.tsx';

const fileFilter = {
  filters: [{ name: 'sql', extensions: ['sql', 'cql', 'txt'] }],
};

function SqlTransaction(props: { conn: IQueryConn; nonce; executeTimes }) {
  const { conn, nonce, executeTimes } = props;
  const [commit, setCommit] = useState(true);
  const { showNotification } = useContext(NotificationContext);
  const [txCount, setTxCount] = useState(0);

  useEffect(() => {
    setTxCount((e) => e + 1);
  }, [executeTimes]);
  const setCloseConfirm = useContext(SetCloseConfirmTextContext);
  const handleQueryCommit = async () => {
    try {
      let c = await conn.getAutoCommit();
      setCommit(c);
    } catch (e) {
      showNotification('error', 'query commit failed', e);
    }
  };
  useEffect(() => {
    setTxCount(0);
    if (commit) {
      setCloseConfirm('');
    } else {
      setCloseConfirm(
        'In Transaction,better commit or rollback.uncommit transaction may lead database locks'
      );
    }
  }, [commit]);

  useEffect(() => {
    handleQueryCommit();
  }, []);

  useEffect(() => {
    handleQueryCommit();
  }, [nonce]);

  const handleCommit = async () => {
    try {
      await conn.commit();
      showNotification('success', 'commit ok');
      handleQueryCommit();
    } catch (e) {
      showNotification('error', 'commit failed', e);
    }
  };

  const handleRollback = async () => {
    try {
      await conn.rollback();
      showNotification('success', 'rollback ok');
      handleQueryCommit();
    } catch (e) {
      showNotification('error', 'rollback failed', e);
    }
  };
  const handleReverse = async () => {
    try {
      await conn.setAutoCommit(!commit);
      setCommit((e) => !e);
      showNotification('success', 'set auto commit ok');
    } catch (e) {
      showNotification('error', 'set commit failed', e);
    }
  };

  return (
    <>
      <Tooltip title="reverse commit mode" arrow>
        <Chip
          onClick={handleReverse}
          label={`${commit ? 'auto' : 'manual'}-commit`}
          size="small"
          color={commit ? 'info' : 'success'}
        ></Chip>
      </Tooltip>

      {!commit && (
        <>
          <Tooltip title="executes after begin transaction" arrow>
            <Chip label={txCount} size="small" color="info"></Chip>
          </Tooltip>

          <Tooltip title="commit" arrow>
            <IconButton onClick={handleCommit} color="primary">
              <FaUpload fontSize="1.1rem"></FaUpload>
            </IconButton>
          </Tooltip>
          <Tooltip title="rollback" arrow>
            <IconButton onClick={handleRollback} color="primary">
              <MdOutlineReply fontSize="1.7rem"></MdOutlineReply>
            </IconButton>
          </Tooltip>
        </>
      )}
    </>
  );
}

function SqlQuery(props: {
  uuid: string;
  handelTextAndPositionChange?: (text: string, pos: Position) => void;
  makeConn: MakeIQueryConn;
  value?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  favoriteName?: string;
  kind: DataSourceKind;
  buttons?: ReactNode;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
}) {
  let {
    handelTextAndPositionChange,
    uuid,
    makeConn,
    value,
    saveFavoriteSql,
    favoriteName: oldFavoriteName,
    kind,
    buttons,
    getParseErr,
  } = props;
  const [resp, setResp] = useState([]);
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);
  const [parseErr, setParseErr] = useState<ErrAndLoc | null>(null);
  let editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [showSuggestReconn, setShowSuggestReconn] = useState<number | null>(
    null
  );
  const queryLogHandler = useRef(new QueryLogHandler(uuid));
  const [executeTimes, setExecuteTimes] = useState(0);

  const [commitNonce, setCommitNonce] = useState(0);
  const [file, setFile] = useState('');
  const [conn, setConn] = useState<IQueryConn | null>(null);
  const [showExport, setShowExport] = useState(false);

  const handleEditorDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, _monaco) => {
      editor.current = ed;
      ed.onDidChangeModelContent((_event) => {
        let text = ed.getValue();
        if (handelTextAndPositionChange) {
          handelTextAndPositionChange(text, ed.getPosition());
        }
        getParseErr(text).then((e) => {
          setParseErr(e);
        });
      });

      ed.onDidChangeCursorPosition((_event) => {
        let p = ed.getPosition();
        setPos(p);
        if (handelTextAndPositionChange) {
          handelTextAndPositionChange(ed.getValue(), p);
        }
      });
    },
    []
  );

  const handleReconnect = async () => {
    setLoading(false);
    if (conn) {
      const confirm = await secondConfirm({
        title: 'reconnection',
        content: 'unsaved data may lost.',
        confirmText: 'Connect',
      });
      if (!confirm) {
        return;
      }
    }
    if (conn) {
      conn
        .reconnect()
        .then(() => {
          showNotification('success', 'Reconnect OK');
        })
        .catch((e) => {
          showNotification('error', 'Reconnect Failure', e);
        })
        .finally(() => {
          setLoading(false);
          setCommitNonce((e) => e + 1);
        });
    } else {
      makeConn()
        .then((conn) => {
          setConn(conn);
          showNotification('success', 'Reconnect OK');
        })
        .catch((e) => {
          showNotification('error', 'Reconnect Failure', e);
        })
        .finally(() => {
          setLoading(false);
          setCommitNonce((e) => e + 1);
        });
    }
  };

  const warnDangerousSql = async (text): Promise<boolean> => {
    if (await conn.isDangerSql(text)) {
      return await secondConfirm({
        confirmText: 'Execute',
        title: 'This is a dangerous sql',
        content: `Are u sure going to execute '${text}'`,
      });
    } else {
      return true;
    }
  };

  const suggestReconnect = (start: Date) => {
    //@ts-ignore
    let queryTime = new Date() - start;
    if (queryTime > 5_000) {
      setShowSuggestReconn(queryTime as number);
      setTimeout(() => {
        setShowSuggestReconn(null);
      }, msgTimeout);
    }
  };

  const handleExecute = async () => {
    const text = getEditorText(editor.current, showNotification);
    if (!text) {
      return;
    }
    let confirm;
    try {
      confirm = await warnDangerousSql(text);
    } catch (_e) {
      console.log('warn about dangerous sql error', _e);
      confirm = true;
    }
    if (!confirm) {
      return;
    }
    let now = new Date();
    setLoading(true);
    setExecuteTimes((e) => e + 1);
    conn
      .executeOP(text)
      .then((e) => {
        queryLogHandler.current.save(text, rustResultOk(e), showNotification);
        setResp([e]);
        suggestReconnect(now);
      })
      .catch((e) => {
        setResp([]);
        showNotification('error', 'query failed', e);
      })
      .finally(() => {
        setLoading(false);
        setCommitNonce((e) => e + 1);
      });
  };
  const handleQuery = async (append: boolean, explain: boolean = false) => {
    let text = getEditorText(editor.current, showNotification);
    if (!text) {
      return;
    }
    if (explain) {
      text = `EXPLAIN ${text}`;
    }

    let confirm;
    try {
      confirm = await warnDangerousSql(text);
    } catch (e) {
      console.log('warnDangerousSql failed : err:', e);
      confirm = true;
    }
    if (!confirm) {
      return;
    }
    let now = new Date();
    setLoading(true);
    setExecuteTimes((e) => e + 1);
    conn
      .queryOP(text)
      .then((resp) => {
        queryLogHandler.current.save(
          text,
          rustResultOk(resp),
          showNotification
        );
        if (append) {
          setResp((p) => [...p, ...resp]);
        } else {
          setResp(resp);
        }
        suggestReconnect(now);
        showNotification('success', 'Execute OK');
      })
      .catch((e) => {
        setResp([]);
        showNotification('error', 'query failed', e);
      })
      .finally(() => {
        setLoading(false);
        setCommitNonce((e) => e + 1);
      });
  };

  useEffect(() => {
    makeConn()
      .then((conn) => {
        setConn((prev) => {
          if (prev) {
            prev.destroy();
          }
          return conn;
        });
      })
      .catch((e) => {
        showNotification('error', 'make connection failed', e);
      });
  }, []);
  useEffect(() => {
    return () => {
      if (conn) {
        conn.destroy();
      }
    };
  }, [conn]);
  const setting = useContext(SettingContext);
  const eidtorOptions =
    useMemo((): editor.IStandaloneEditorConstructionOptions => {
      return {
        fontSize: setting.fontSize,
        fontWeight: setting.fontWeight,
        fontFamily: setting.fontFamily,
        tabSize: setting.tabSize,
        ...setting.editorOption,
      };
    }, [setting]);

  const editorDivDom = useRef(null);
  useEffect(() => {
    if (editorDivDom.current && conn) {
      (editorDivDom.current as HTMLDivElement).addEventListener(
        'keydown',
        function (event) {
          if (event.ctrlKey && event.key.toLowerCase() === 'e') {
            event.preventDefault();
            handleQuery(false);
          }
        }
      );
    }
  }, [conn]);

  const [size, setSize] = useState(400);
  const sizeKey = 'query-size';
  useEffect(() => {
    SaveSizesToStore.getSize(sizeKey).then((s) => {
      if (s && s.height) {
        setSize(s.height);
      }
    });
  }, []);
  const vis = useContext(VisiableContext);
  useEffect(() => {
    if (vis && conn) {
      console.log('SqlQuery setQueryId', conn.queryId);
      invoke('set_auto_completion_query_id', { queryId: conn.queryId })
        .then(() => {})
        .catch((e) => {
          console.log('set auto completion id failed,err:{}', e);
        });
    }
  }, [vis, conn]);

  const keyEscCancelQueryLog = useCallback(
    (e) => {
      if (e.key === 'Escape' && vis && showQueryLog) {
        setShowQueryLog(false);
        e.preventDefault();
      }
    },
    [vis, showQueryLog]
  );

  useEffect(() => {
    document.addEventListener('keydown', keyEscCancelQueryLog);
    return () => {
      document.removeEventListener('keydown', keyEscCancelQueryLog);
    };
  }, [keyEscCancelQueryLog]);

  const supportExplain = [
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.Oracle,
    DataSourceKind.MsSql,
    DataSourceKind.Postgres,
    DataSourceKind.ClickHouse,
    DataSourceKind.FireBird,
  ].includes(kind);

  return (
    //@ts-ignore
    <SplitPane
      split="horizontal"
      defaultSize={500}
      minSize={200}
      size={size}
      style={{ position: 'relative' }}
      onChange={(newvalue) => {
        SaveSizesToStore.saveSize(sizeKey, { height: newvalue });
      }}
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          width: '100%',
        }}
      >
        <QueryLog
          showQueryLog={showQueryLog}
          setShowQueryLog={setShowQueryLog}
          uuid={uuid}
          executeTimes={executeTimes}
        ></QueryLog>

        <Stack
          direction="row"
          spacing={1}
          sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        >
          <Tooltip title="execute" arrow>
            <IconButton
              loading={loading}
              disabled={loading || conn == null}
              onClick={() => {
                handleQuery(false);
              }}
              color="primary"
            >
              <PlayArrowIcon />
            </IconButton>
          </Tooltip>

          <Tooltip title="execute and append result to tabs" arrow>
            <IconButton
              loading={loading}
              disabled={loading || conn == null}
              onClick={() => {
                handleQuery(true);
              }}
              color="primary"
            >
              <FastForwardIcon />
            </IconButton>
          </Tooltip>

          <Tooltip title="execute sql not expecting result" arrow>
            <IconButton
              loading={loading}
              disabled={loading || conn == null}
              onClick={handleExecute}
              color="primary"
            >
              <FaExclamation />
            </IconButton>
          </Tooltip>
          {supportExplain && (
            <Tooltip title="explain sql" arrow>
              <IconButton
                loading={loading}
                disabled={loading || conn == null}
                onClick={() => {
                  handleQuery(false, true);
                }}
                color="primary"
              >
                <BsFillLightbulbFill />
              </IconButton>
            </Tooltip>
          )}

          {conn && conn.supportTransaction() && (
            <SqlTransaction
              nonce={commitNonce}
              executeTimes={executeTimes}
              conn={conn}
            />
          )}
          <Tooltip
            title={
              showSuggestReconn
                ? `query used ${showSuggestReconn}ms,try reconnect?`
                : 'reconnect'
            }
            arrow
          >
            <IconButton
              onClick={() => {
                handleReconnect();
              }}
              color="primary"
            >
              <RefreshIcon color={showSuggestReconn ? 'error' : 'inherit'} />
            </IconButton>
          </Tooltip>

          <Tooltip title="export query result to a file" arrow>
            <IconButton
              loading={loading}
              disabled={loading || conn == null}
              onClick={() => {
                const value = getEditorText(editor.current, showNotification);
                if (!value) {
                  return;
                }
                setShowExport(true);
              }}
              color="primary"
            >
              <ArrowCircleDownRoundedIcon />
            </IconButton>
          </Tooltip>
          <Tooltip
            arrow
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
              <FaBookmark style={{ fontSize: '1.2rem' }} />
            </IconButton>
          </Tooltip>

          <LoadStoreFileForQuery
            file={file}
            getQuery={() => {
              return editor.current.getValue();
            }}
            handleChange={(file, content) => {
              setFile(file);
              editor.current.setValue(content);
            }}
            openOpt={{ ...fileFilter }}
            saveOpt={{ ...fileFilter }}
          ></LoadStoreFileForQuery>
          <Tooltip title="query log" arrow>
            <IconButton
              onClick={() => {
                setShowQueryLog((prev) => !prev);
              }}
              color="primary"
              disabled={conn == null}
            >
              <EventNoteIcon></EventNoteIcon>
            </IconButton>
          </Tooltip>
          {buttons}

          <ExecuteTime loading={loading} disableTime></ExecuteTime>
        </Stack>
        <div
          style={{
            flexGrow: 1,
            flexShrink: 1,
            minHeight: 0,
            height: '400px',
          }}
          ref={editorDivDom}
        >
          <Editor
            language={uuid}
            onMount={handleEditorDidMount}
            width="99%"
            height="99%"
            value={value}
            options={eidtorOptions}
            theme={setting.editorTheme()}
          ></Editor>
        </div>
        <EditorBar
          left={
            <Stack
              style={{
                flex: 1,
              }}
              direction="row"
            >
              {parseErr && (
                <PossibleParseError
                  parseErr={parseErr}
                  editor={editor.current}
                ></PossibleParseError>
              )}
            </Stack>
          }
          pos={pos}
        ></EditorBar>
      </div>

      <div>
        <MultiQueryResp data={resp}></MultiQueryResp>

        {showExport && (
          <Modal
            open={showExport}
            onClose={(e: EscDownEvent, reason) => {
              if (reason === 'backdropClick') {
                return;
              }

              e.preventDefault();
              setShowExport(false);
            }}
          >
            <Box sx={modalStyle} width="45%">
              <ExportView
                supportKind={['csv', 'json', 'jsonline']}
                exportOP={async (kind, file) => {
                  try {
                    const sql = getEditorText(editor.current, showNotification);
                    await conn.exportQueryResult(sql, kind, file);
                    showNotification('success', 'export OK');
                  } catch (e) {
                    showNotification('error', 'export failure', e);
                  }
                }}
                name="export_query_result"
                supportCsvHeader={true}
              ></ExportView>
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
            <Box sx={modalStyle} width="40%">
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
                    <FaBookmark> </FaBookmark>
                  </IconButton>
                </Stack>
              </Stack>
            </Box>
          </Modal>
        )}
      </div>
    </SplitPane>
  );
}

function rustResultOk(x: any): boolean {
  if (x instanceof Array) {
    for (let i = 0; i < x.length; i++) {
      if (!rustResultOk(x[i])) {
        return false;
      }
    }
    return true;
  }
  return !!x['Ok'];
}
export { SqlQuery };
