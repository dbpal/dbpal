'use strict';
import { Editor } from '@monaco-editor/react';
import { MultiQueryResp } from './QueryResp';
import EventNoteIcon from '@mui/icons-material/EventNote';
import {
  Box,
  Chip,
  CircularProgress,
  IconButton,
  Modal,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';

import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { FaExclamation } from 'react-icons/fa';
import {
  ReactNode,
  RefObject,
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
  EscDownEvent,
  IQueryConn,
  MakeIQueryConn,
  modalHandleClose,
  modalStyle,
  msgTimeout,
  NameAndQueryId,
  SetAutoCompletionName,
} from './Common.ts';
import { BsFillLightbulbFill } from 'react-icons/bs';
import { FaBookmark } from 'react-icons/fa';
import { FaUpload } from 'react-icons/fa';
import { MdOutlineReply } from 'react-icons/md';
import { NotificationContext } from './Notification.tsx';
import { QueryLog, QueryLogKind } from './QueryLog.tsx';
import RefreshIcon from '@mui/icons-material/Refresh';
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  QueryLogHandler,
  rustResultOk,
  secondConfirm,
  SetCloseConfirmTextContext,
  SettingContext,
  TabActiveContext,
} from './helper.ts';
import ArrowCircleDownRoundedIcon from '@mui/icons-material/ArrowCircleDownRounded';
import { ExportView } from './Export.tsx';
import { ExecutingTime, PossibleParseError } from './CommonComp.tsx';
import { EditorBar, LoadStoreFileForQuery } from './CommonComp2.tsx';
function SqlTransaction(props: { conn: IQueryConn; executeTimes: number }) {
  const { conn, executeTimes } = props;
  const [commit, setCommit] = useState(true);
  const { showNotification } = useContext(NotificationContext);
  const [txCount, setTxCount] = useState(0);

  const setCloseConfirm = useContext(SetCloseConfirmTextContext);
  const handleQueryCommit = async () => {
    try {
      let c = await conn.getAutoCommit();
      setCommit(c);
    } catch (e) {
      showNotification('error', 'get commit modal failed', e);
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
    //这里的顺序很重要
    setTxCount((e) => e + 1);
    handleQueryCommit();
  }, [executeTimes]);

  const commitTx = async () => {
    try {
      const confirm = await secondConfirm({
        title: 'Commit Transaction',
        confirmText: 'Commit',
      });
      if (!confirm) {
        return;
      }
      await conn.commit();
      showNotification('success', 'commit ok');
      handleQueryCommit();
    } catch (e) {
      showNotification('error', 'commit failed', e);
    }
  };

  const rollbackTx = async () => {
    try {
      await conn.rollback();
      showNotification('success', 'rollback ok');
      handleQueryCommit();
    } catch (e) {
      showNotification('error', 'rollback failed', e);
    }
  };
  const reverseCommitMode = async () => {
    try {
      await conn.setAutoCommit(!commit);
      setCommit((e) => !e);
      showNotification('success', 'set commit mode ok');
    } catch (e) {
      showNotification('error', 'set commit failed', e);
    }
  };

  return (
    <>
      <Tooltip title="reverse commit mode">
        <Chip
          onClick={reverseCommitMode}
          label={`${commit ? 'auto' : 'manual'}-commit`}
          color={commit ? 'info' : 'success'}
          variant={commit ? 'outlined' : 'filled'}
        ></Chip>
      </Tooltip>

      {!commit && (
        <>
          <Tooltip title="executes after begin transaction">
            <Chip label={txCount} color="info"></Chip>
          </Tooltip>

          <Tooltip title="commit">
            <IconButton onClick={commitTx} color="primary">
              <FaUpload fontSize="1.1rem"></FaUpload>
            </IconButton>
          </Tooltip>
          <Tooltip title="rollback">
            <IconButton onClick={rollbackTx} color="primary">
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
  makeConn: MakeIQueryConn;
  value?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  favoriteName?: string;
  kind: DataSourceKind;
  buttons?: ReactNode;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
  editorRef?: RefObject<editor.IStandaloneCodeEditor>;
  height?: string;
}) {
  let {
    uuid,
    makeConn,
    value,
    saveFavoriteSql,
    favoriteName: oldFavoriteName,
    kind,
    buttons,
    getParseErr,
    editorRef,
    height,
  } = props;
  const [resp, setResp] = useState([]);
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);
  const [parseErr, setParseErr] = useState<ErrAndLoc | null>(null);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [showSuggestReconnect, setShowSuggestReconnect] = useState<
    number | null
  >(null);
  const setting = useContext(SettingContext);
  const queryLogHandler = useRef(new QueryLogHandler(uuid));
  const [executeTimes, setExecuteTimes] = useState(0);
  const [conn, setConn] = useState<IQueryConn | null>(null);
  const [showExport, setShowExport] = useState(false);

  const handleEditorDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, _monaco) => {
      setEditor(ed);
      if (editorRef) {
        editorRef.current = ed;
      }

      ed.onDidChangeModelContent((_event) => {
        let text = ed.getValue();
        // getParseErr(text).then((e) => {
        //   setParseErr(e);
        // });
      });

      ed.onDidChangeCursorPosition((_event) => {
        let p = ed.getPosition();
        setPos(p);
      });
    },
    []
  );

  const handleReconnect = async () => {
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
          // trigger some reload like sqltransaction
          setExecuteTimes((e) => e + 1);
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
          setExecuteTimes((e) => e + 1);
        });
    }
  };
  const warnDangerousSql = async (text): Promise<boolean> => {
    try {
      if (await conn.isDangerSql(text)) {
        return await secondConfirm({
          confirmText: 'Execute',
          title: 'This is a dangerous sql',
          content: `Are u sure going to execute!!!!'`,
        });
      } else {
        return true;
      }
    } catch (e) {
      console.log('warning dangerous failed,error:', e);
      return true;
    }
  };

  const suggestReconnect = (start: Date) => {
    //@ts-ignore
    let queryTime = new Date() - start;
    if (queryTime > 5_000) {
      setShowSuggestReconnect(queryTime as number);
      setTimeout(() => {
        setShowSuggestReconnect(null);
      }, msgTimeout);
    }
  };

  const handleExecute = async () => {
    const text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    let confirm = await warnDangerousSql(text);
    if (!confirm) {
      return;
    }
    let now = new Date();
    setLoading(true);
    conn
      .executeOP(text)
      .then((e) => {
        if (setting.saveQueryLog) {
          queryLogHandler.current.save(text, rustResultOk(e), showNotification);
        }
        setResp([e]);
        suggestReconnect(now);
      })
      .catch((e) => {
        setResp([]);
        showNotification('error', 'query failed', e);
      })
      .finally(() => {
        setExecuteTimes((e) => e + 1);
        setLoading(false);
      });
  };
  const handleQuery = async (append: boolean, explain: boolean = false) => {
    let text = getEditorText(editor, showNotification);
    if (!text) {
      return;
    }
    if (explain) {
      text = `EXPLAIN ${text}`;
    }
    let confirm = await warnDangerousSql(text);
    if (!confirm) {
      return;
    }
    let now = new Date();
    setLoading(true);
    conn
      .queryOP(text)
      .then((resp) => {
        if (setting.saveQueryLog) {
          queryLogHandler.current.save(
            text,
            rustResultOk(resp),
            showNotification
          );
        }

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
        setExecuteTimes((e) => e + 1);
        setLoading(false);
      });
  };

  // make connection for the first time
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
  // destroy connection
  useEffect(() => {
    return () => {
      if (conn) {
        conn.destroy();
      }
    };
  }, [conn]);

  const editorOptions =
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
  useEffect(() => {
    if (conn && editor) {
      SetAutoCompletionName.set(
        editor.getModel().id,
        new NameAndQueryId(uuid, conn.queryId)
      );
    }
  }, [conn, editor]);
  const vis = useContext(TabActiveContext);
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
    <PanelGroup
      direction="vertical"
      style={{
        position: 'relative',
        height: height ? height : '90vh',
        overflow: 'auto',
      }}
      autoSaveId="sql-query"
    >
      <Panel
        minSize={10}
        maxSize={70}
        defaultSize={40}
        style={{
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        <QueryLog
          showQueryLog={showQueryLog}
          setShowQueryLog={setShowQueryLog}
          uuid={uuid}
          executeTimes={executeTimes}
          kind={
            kind == DataSourceKind.Redis ? QueryLogKind.Redis : QueryLogKind.Sql
          }
          language={kind}
        ></QueryLog>

        <Stack
          direction="row"
          spacing={1}
          sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        >
          {conn && (
            <>
              <Tooltip title="query">
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

              <Tooltip title="query and append result to results tabs">
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

              <Tooltip title="execute sql not expecting result">
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
                <Tooltip title="explain sql">
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
            </>
          )}

          {conn && conn.supportTransaction() && (
            <SqlTransaction executeTimes={executeTimes} conn={conn} />
          )}
          <Tooltip
            title={
              showSuggestReconnect
                ? `query used ${showSuggestReconnect}ms,try reconnect?`
                : 'reconnect'
            }
          >
            <IconButton
              onClick={() => {
                handleReconnect();
              }}
              color="primary"
            >
              <RefreshIcon color={showSuggestReconnect ? 'error' : 'inherit'} />
            </IconButton>
          </Tooltip>
          {conn && (
            <Tooltip title="export query result to a file,implemented it not using query connection,uncommit row may not be found">
              <IconButton
                loading={loading}
                disabled={loading || conn == null}
                onClick={() => {
                  const value = getEditorText(editor, showNotification);
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
          )}

          <Tooltip
            title={
              !favoriteName
                ? 'save sql to favorite.'
                : `save favorite sql as named '${favoriteName}'`
            }
          >
            <IconButton
              onClick={() => {
                const value = editor.getValue();
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
          {editor && (
            <LoadStoreFileForQuery
              getQuery={() => {
                return editor.getValue();
              }}
              handleChange={(content) => {
                editor.setValue(content);
              }}
              kind={kind}
            ></LoadStoreFileForQuery>
          )}

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
          {buttons}

          <ExecutingTime loading={loading} disableTime></ExecutingTime>
          {!conn && <CircularProgress></CircularProgress>}
        </Stack>
        <div
          style={{
            flex: 1,
            minHeight: 0,
            height: '100px',
          }}
          ref={editorDivDom}
        >
          <Editor
            language={kind}
            onMount={handleEditorDidMount}
            width="99%"
            height="99%"
            value={value}
            options={editorOptions}
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
                  editor={editor}
                ></PossibleParseError>
              )}
            </Stack>
          }
          pos={pos}
        ></EditorBar>
      </Panel>
      <PanelResizeHandle style={{ height: 4 }} className="resize-handle" />
      <Panel style={{ overflow: 'auto' }}>
        <div style={{ height: '100%' }}>
          <MultiQueryResp data={resp}></MultiQueryResp>
          {showExport && (
            <Modal
              open={showExport}
              onClose={modalHandleClose((e: EscDownEvent) => {
                e.preventDefault();
                setShowExport(false);
              })}
            >
              <Box sx={modalStyle} width="45%">
                <ExportView
                  supportKind={['csv', 'json', 'jsonline', 'xlsx']}
                  exportOP={async (kind, file) => {
                    const sql = getEditorText(editor, showNotification);
                    return conn.exportQueryResult(sql, kind, file);
                  }}
                  name="export_query_result"
                ></ExportView>
              </Box>
            </Modal>
          )}
          {showSetFavoriteName && (
            <Modal
              open={showSetFavoriteName}
              onClose={modalHandleClose((e: EscDownEvent) => {
                e.preventDefault();
                setShowFavoriteName(false);
              })}
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
                        saveFavoriteSql(favoriteName, editor.getValue());
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
      </Panel>
    </PanelGroup>
  );
}

export { SqlQuery };
