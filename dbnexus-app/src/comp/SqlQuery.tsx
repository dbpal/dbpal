import { Editor } from '@monaco-editor/react';
import SplitPane from 'react-split-pane';
import { MultiQueryResp } from './QueryResp';

import './style/react-split-pane.css';
import {
  Button,
  CircularProgress,
  IconButton,
  Paper,
  Stack,
  Tooltip,
} from '@mui/material';

import CloseIcon from '@mui/icons-material/Close';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import PlayCircleIcon from '@mui/icons-material/PlayCircle';
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';
import { editor, Position } from 'monaco-editor';
import {
  EditorBar,
  IQueryConn,
  MakeIQueryConn,
  msgTimeout,
  progressBarSize,
  SettingsContext,
} from './Common.tsx';
import { NotificationContext } from './Notification.tsx';
import { QueryLog } from './QueryLog.tsx';
import RefreshIcon from '@mui/icons-material/Refresh';
import { secondConfirm } from './helper.ts';

function OracleCommit(props: { conn: IQueryConn }) {
  const { conn } = props;
  const [commit, setCommit] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleQueryCommit = async () => {
    try {
      setCommit(await conn.oracleQueryAutocommit());
    } catch (e) {
      showNotification('error', 'query oracle commit failed', e);
    }
  };
  useEffect(() => {
    handleQueryCommit();
  }, []);
  const handleCommit = async () => {
    try {
      await conn.oracleQueryCommit();
      showNotification('success', 'commit ok');
    } catch (e) {
      showNotification('error', 'oracle commit failed', e);
    }
  };
  const handleRollback = async () => {
    try {
      await conn.oracleQueryCommit();
      showNotification('success', 'rollback ok');
    } catch (e) {
      showNotification('error', 'oracle commit failed', e);
    }
  };
  const handleReverse = async () => {
    try {
      await conn.oracleQuerySetAutocommit(!commit);
      setCommit((e) => !e);
      showNotification('success', 'set auto commit ok');
    } catch (e) {
      showNotification('error', 'oracle set commit failed', e);
    }
  };
  return (
    <>
      <Tooltip title="reverse commit mode">
        <Button onClick={handleReverse}>
          {commit ? 'auto' : 'Manual'}-commit
        </Button>
      </Tooltip>
      {!commit && (
        <>
          <Button onClick={handleCommit}>commit </Button>
          <Button onClick={handleRollback}>rollback </Button>
        </>
      )}
    </>
  );
}

function SqlQuery(props: {
  language: string;
  handelTextAndPositionChange?: (text: string, pos: Position) => void;
  connectInfo?: ReactNode;
  makeConn: MakeIQueryConn;
}) {
  let { handelTextAndPositionChange, language, connectInfo, makeConn } = props;
  const [resp, setResp] = useState([]);
  let editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [queryLogs, setQueryLogs] = useState([]);
  const [showSuggestReconn, setShowSuggestReconn] = useState<number | null>(
    null
  );
  const [conn, setConn] = useState<IQueryConn | null>(null);
  const { theme } = useContext(SettingsContext);
  const handleEditorDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, monaco) => {
      editor.current = ed;
      if (theme == 'dark') {
        monaco.editor.setTheme('vs-dark');
      } else {
        monaco.editor.setTheme('light');
      }
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
    if (conn) {
      const confirm = await secondConfirm({
        title: 're connection',
        content: 'Maybe unsaved data will lost.',
        confirmText: 'connect',
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
        });
    } else {
      makeConn()
        .then((conn) => {
          setConn(conn);
          showNotification('success', 'Reconnect OK');
        })

        .catch((e) => {
          showNotification('error', 'Reconnect Failure', e);
        });
    }
  };

  const handleQuery = async (text: string) => {
    if (!text.trim()) {
      showNotification('error', `can't execute empty statement.`);
      return;
    }
    try {
      if (await conn.isDangerSql(text)) {
        const confirm = await secondConfirm({
          confirmText: 'execute',
          title: 'This is a dangerous sql',
          content: `Are u sure going to execute '${text}'`,
        });
        if (!confirm) {
          return;
        }
      }
    } catch (e) {
      console.log(
        'isDangerSql has error ,maybe not a valid sql,should be fine to continue',
        e
      );
    }

    let now = new Date();
    setLoading(true);
    conn
      .queryOP(text)
      .then((resp) => {
        setResp(resp);
        setLoading(false);
        if (showQueryLog) {
          handleLoadQueryLog();
        }
        //@ts-ignore
        let queryTime = new Date() - now;
        if (queryTime > 5_000) {
          setShowSuggestReconn(queryTime as number);
          setTimeout(() => {
            setShowSuggestReconn(null);
          }, msgTimeout);
        }
        if (showQueryLog) {
          handleLoadQueryLog();
        }
        showNotification('success', 'Execute OK');
      })
      .catch((e) => {
        setLoading(false);
        setResp([]);
        showNotification('error', 'query failed', e);
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
        {/* // here is query log use positive to locate. */}
        {showQueryLog && (
          <Paper
            style={{
              position: 'absolute',
              opacity: 1,
              right: 0,
              top: 0,
              display: showQueryLog ? 'block' : 'none',
              zIndex: 13,
              width: '65%',
            }}
          >
            <Stack direction="row" sx={{ justifyContent: 'flex-start' }}>
              <Button
                onClick={() => {
                  setShowQueryLog(false);
                }}
              >
                <CloseIcon></CloseIcon>
              </Button>
            </Stack>
            <QueryLog language={language} logs={queryLogs}></QueryLog>
          </Paper>
        )}
        {connectInfo}
        <Stack direction="row" spacing={1}>
          {loading && <CircularProgress size={progressBarSize} />}
          <Tooltip title="execute">
            <IconButton
              disabled={loading || conn == null}
              onClick={() => {
                handleQuery(editor.current.getValue());
              }}
              color="primary"
              aria-label="execute"
            >
              <PlayArrowIcon />
            </IconButton>
          </Tooltip>
          <Tooltip title="execute selected">
            <IconButton
              disabled={loading || conn == null}
              onClick={() => {
                handleQuery(
                  editor.current
                    .getModel()
                    .getValueInRange(editor.current.getSelection())
                );
              }}
              color="primary"
              aria-label="execute"
            >
              <PlayCircleIcon />
            </IconButton>
          </Tooltip>

          <Tooltip
            title={
              showSuggestReconn
                ? `query used ${showSuggestReconn}ms,try reconnect?`
                : 'reconnect'
            }
          >
            <IconButton
              disabled={loading}
              onClick={() => {
                handleReconnect();
              }}
              color="primary"
              aria-label="execute"
            >
              <RefreshIcon color={showSuggestReconn ? 'error' : 'inherit'} />
            </IconButton>
          </Tooltip>
          {conn && conn.isOracle && <OracleCommit conn={conn} />}
          <Button
            onClick={() => {
              setShowQueryLog((prev) => !prev);
            }}
            color={showQueryLog ? 'error' : 'info'}
            disabled={conn == null}
          >
            {showQueryLog ? <CloseIcon></CloseIcon> : ''} query log
          </Button>
        </Stack>
        <div
          style={{
            flexGrow: 1,
            flexShrink: 1,
            minHeight: 0,
            height: '400px',
            width: '99%',
          }}
        >
          <Editor
            language={language}
            onMount={handleEditorDidMount}
            value=""
            options={{
              fontSize: useContext(SettingsContext).editorFontSize,
              wordWrap: 'on',
            }}
          ></Editor>
        </div>
        <EditorBar
          style={{ paddingBottom: '10px', flexGrow: 0, flexShrink: 0 }}
          pos={pos}
        ></EditorBar>
      </div>
      <div>
        <MultiQueryResp data={resp}></MultiQueryResp>
      </div>
    </SplitPane>
  );
}

export { SqlQuery };
