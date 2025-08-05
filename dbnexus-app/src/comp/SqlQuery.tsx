import { Editor } from '@monaco-editor/react';
import SplitPane from 'react-split-pane';
import { MultiQueryResp } from './QueryResp';
import EventNoteIcon from '@mui/icons-material/EventNote';
import './style/react-split-pane.css';
import {
  Box,
  Button,
  Chip,
  IconButton,
  Modal,
  Paper,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';

import FavoriteIcon from '@mui/icons-material/Favorite';
import CloseIcon from '@mui/icons-material/Close';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import { FaExclamation } from 'react-icons/fa';
import {
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
  SettingContext,
} from './Common.tsx';
import { FaBookmark } from 'react-icons/fa';
import { FaUpload } from 'react-icons/fa';
import { MdOutlineReply } from 'react-icons/md';
import { NotificationContext } from './Notification.tsx';
import { QueryLog } from './QueryLog.tsx';
import RefreshIcon from '@mui/icons-material/Refresh';
import { secondConfirm } from './helper.ts';
import ArrowCircleDownRoundedIcon from '@mui/icons-material/ArrowCircleDownRounded';
import { ExportView } from './Export.tsx';

import { IoIosTime } from 'react-icons/io';
const fileFilter = {
  filters: [{ name: 'sql', extensions: ['sql', 'cql', 'txt'] }],
};

function SqlTransaction(props: { conn: IQueryConn; nonce }) {
  const { conn, nonce } = props;
  const [commit, setCommit] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [txCount, setTxCount] = useState(0);

  const handleQueryCommit = async () => {
    try {
      let c = await conn.getAutoCommit();
      setCommit(c);
    } catch (e) {
      showNotification('error', 'query commit failed', e);
    }
  };

  const handleReloadTxCount = async () => {
    conn
      .txExecuteCounts()
      .then((count) => {
        setTxCount(count);
      })
      .catch((e) => {
        showNotification('error', 'query tx count failed', e);
      });
  };

  useEffect(() => {
    handleQueryCommit();
  }, []);

  useEffect(() => {
    handleQueryCommit();
    handleReloadTxCount();
  }, [nonce]);

  const handleCommit = async () => {
    try {
      await conn.commit();
      showNotification('success', 'commit ok');
      handleQueryCommit();
      handleReloadTxCount();
    } catch (e) {
      showNotification('error', 'commit failed', e);
    }
  };

  const handleRollback = async () => {
    try {
      await conn.rollback();
      showNotification('success', 'rollback ok');
      handleQueryCommit();
      handleReloadTxCount();
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
      <Tooltip title="reverse commit mode">
        <Chip
          onClick={handleReverse}
          label={`${commit ? 'auto' : 'manual'}-commit`}
          size="small"
          color={commit ? 'info' : 'success'}
        ></Chip>
      </Tooltip>

      {!commit && (
        <>
          <Tooltip title="executes after begin transaction">
            <Chip label={txCount} size="small" color="info"></Chip>
          </Tooltip>

          <Tooltip title="commit">
            <IconButton onClick={handleCommit} color="primary">
              <FaUpload fontSize="1.1rem"></FaUpload>
            </IconButton>
          </Tooltip>
          <Tooltip title="rollback">
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
  language: string;
  handelTextAndPositionChange?: (text: string, pos: Position) => void;

  makeConn: MakeIQueryConn;
  value?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  favoriteName?: string;
}) {
  let {
    handelTextAndPositionChange,
    language,
    makeConn,
    value,
    saveFavoriteSql,
    favoriteName: oldFavoriteName,
  } = props;
  const [resp, setResp] = useState([]);
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);

  let editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [queryLogs, setQueryLogs] = useState([]);
  const [showSuggestReconn, setShowSuggestReconn] = useState<number | null>(
    null
  );

  const executeTimeRef = useRef(null);

  const [commitNonce, setCommitNonce] = useState(0);
  const [file, setFile] = useState('');
  const [conn, setConn] = useState<IQueryConn | null>(null);
  const [showExport, setShowExport] = useState(false);
  const handleEditorDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, _monaco) => {
      editor.current = ed;
      if (handelTextAndPositionChange) {
        ed.onDidChangeModelContent((_event) => {
          let text = ed.getValue();
          handelTextAndPositionChange(text, ed.getPosition());
        });
      }
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

  const handleLoadQueryLog = () => {
    conn
      .loadQueryLog()
      .then((logs) => {
        setQueryLogs(logs);
      })
      .catch((e) => {
        showNotification('error', 'fetch query logs failed', e);
      });
  };
  useEffect(() => {
    if (showQueryLog) {
      handleLoadQueryLog();
    }
  }, [showQueryLog]);
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

  const getText = () => {
    const range = editor.current.getSelection();
    let text: string;
    if (range) {
      text = editor.current.getModel().getValueInRange(range);
    }
    if (!text) {
      text = editor.current.getValue();
    }
    text = text.trim();
    if (!text) {
      showNotification('error', 'empty statement');
      return undefined;
    }
    return text;
  };
  const timeExecute = () => {
    const now = new Date();
    const setUp = () => {
      //@ts-ignore
      const ms = new Date() - now;
      function getTime() {
        let s = Math.floor(ms / 1000);
        if (s == 0) {
          return `${ms}ms`;
        }
        let r = ms % 1000;
        if (r == 0) {
          return `${s}s`;
        }
        return `${s}s ${r}ms`;
      }
      (executeTimeRef.current as HTMLDivElement).innerHTML = `${getTime()}`;
    };
    const int = setInterval(() => {
      setUp();
    }, 50);
    return () => {
      clearInterval(int);
      setUp();
    };
  };

  const handleExecute = async () => {
    const text = getText();
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
    const timeExecuteClean = timeExecute();
    conn
      .executeOP(text)
      .then((e) => {
        setResp([e]);
        if (showQueryLog) {
          handleLoadQueryLog();
        }
        suggestReconnect(now);
      })
      .catch((e) => {
        setResp([]);
        showNotification('error', 'query failed', e);
      })
      .finally(() => {
        setLoading(false);
        setCommitNonce((e) => e + 1);
        timeExecuteClean();
      });
  };
  const handleQuery = async (append: boolean) => {
    let text = getText();
    if (!text) {
      return;
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
    const timeExecuteClean = timeExecute();
    conn
      .queryOP(text)
      .then((resp) => {
        if (append) {
          setResp((p) => [...p, ...resp]);
        } else {
          setResp(resp);
        }
        if (showQueryLog) {
          handleLoadQueryLog();
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
        timeExecuteClean();
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
  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.editorFontSize,
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

  return (
    //@ts-ignore
    <SplitPane
      split="horizontal"
      defaultSize={500}
      minSize={50}
      style={{ position: 'relative' }}
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          width: '100%',
        }}
      >
        {/* // here is query log use absolute position to locate. */}
        {showQueryLog && (
          <Paper
            style={{
              position: 'absolute',
              right: 0,
              top: 0,
              display: showQueryLog ? 'block' : 'none',
              zIndex: 13,
              width: '65%',
            }}
          >
            <Stack direction="row">
              <Button
                onClick={() => {
                  setShowQueryLog(false);
                }}
              >
                <CloseIcon></CloseIcon>
              </Button>
            </Stack>
            <QueryLog uuid={language} logs={queryLogs}></QueryLog>
          </Paper>
        )}

        <Stack
          direction="row"
          spacing={1}
          sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        >
          <Tooltip title="execute">
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

          <Tooltip title="execute and append result to tabs">
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
          <Tooltip title="sql execute time">
            <Chip
              color="default"
              size="small"
              label={<span ref={executeTimeRef}> </span>}
              icon={<IoIosTime></IoIosTime>}
            ></Chip>
          </Tooltip>

          {conn && conn.supportTransaction() && (
            <SqlTransaction nonce={commitNonce} conn={conn} />
          )}
          <Tooltip
            title={
              showSuggestReconn
                ? `query used ${showSuggestReconn}ms,try reconnect?`
                : 'reconnect'
            }
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

          <Tooltip title="export query result to a file">
            <IconButton
              loading={loading}
              disabled={loading || conn == null}
              onClick={() => {
                const value = getText();
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
          <Tooltip title="query log">
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
            language={language}
            onMount={handleEditorDidMount}
            width="99.5%"
            value={value}
            options={options}
            theme={setting.editorTheme()}
          ></Editor>
        </div>
        <EditorBar
          style={{ paddingBottom: '10px', flexGrow: 0, flexShrink: 0 }}
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
                handleClose={() => {
                  setShowExport(false);
                }}
                supportKind={['csv', 'json', 'jsonline']}
                exportOP={async (kind, file) => {
                  try {
                    const sql = getText();
                    await conn.exportQueryResult(sql, kind, file);
                    showNotification('success', 'export OK');
                  } catch (e) {
                    showNotification('error', 'export failure', e);
                  }
                }}
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
                    <FavoriteIcon> </FavoriteIcon>
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

export { SqlQuery };
