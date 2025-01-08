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
import { ReactNode, useContext, useEffect, useState } from 'react';
import { editor, Position } from 'monaco-editor';
import {
  EditorBar,
  msgTimeout,
  progressBarSize,
  SettingsContext,
} from './Common.tsx';
import { NotificationContext } from './Notification.tsx';
import types from './ApiStruct.ts';
import { QueryLog } from './QueryLog.tsx';
import RefreshIcon from '@mui/icons-material/Refresh';
function SqlQuery(props: {
  language: string;
  queryOP: (text: string) => Promise<any[]>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
  handelTextAndPositionChange?: (text: string, pos: Position) => void;
  destroy?: () => void;
  reconnect?: () => Promise<void>;
  connectInfo?: ReactNode;
}) {
  let {
    language,
    queryOP,
    loadQueryLog,
    handelTextAndPositionChange,
    destroy,
    reconnect,
    connectInfo,
  } = props;

  const [resp, setResp] = useState([]);
  const [text, setText] = useState('');
  const [selectText, setSelectText] = useState('');
  const [pos, setPos] = useState<Position | null>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [queryLogs, setQueryLogs] = useState([]);
  const [showSuggestReconn, setShowSuggestReconn] = useState<number | null>(
    null
  );
  const { theme } = useContext(SettingsContext);
  const handleEditorDidMount = (
    editor: editor.IStandaloneCodeEditor,
    monaco
  ) => {
    editor.onDidChangeModelContent((_event) => {
      let text = editor.getValue();
      setText(text);
      if (handelTextAndPositionChange) {
        handelTextAndPositionChange(text, editor.getPosition());
      }
    });
    editor.onDidChangeCursorPosition((_event) => {
      let p = editor.getPosition();
      setPos(p);
      if (handelTextAndPositionChange) {
        handelTextAndPositionChange(editor.getValue(), p);
      }
    });
    editor.onDidChangeCursorSelection((_event) => {
      const selection = editor.getSelection();
      setSelectText(editor.getModel().getValueInRange(selection));
    });
    if (theme == 'dark') {
      monaco.editor.setTheme('vs-dark');
    } else {
      monaco.editor.setTheme('light');
    }
  };

  const handleLoadQueryLog = () => {
    loadQueryLog()
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
  const handleReconnect = () => {
    if (!reconnect) {
      return;
    }
    reconnect()
      .then(() => {
        showNotification('success', 'Reconnect OK');
      })

      .catch((e) => {
        showNotification('error', 'Reconnect Failure', e);
      });
  };
  const handleQuery = (text: string) => {
    if (!text.trim()) {
      showNotification('error', `can't execute empty statement.`);
      return;
    }
    let now = new Date();
    setLoading(true);
    queryOP(text)
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
      })
      .catch((e) => {
        setLoading(false);
        setResp([]);
        showNotification('error', 'query failed', e);
      });
  };

  useEffect(() => {
    console.log('mount.......');
    return () => {
      console.log('unmount.......');
      destroy && destroy();
    };
  }, []);
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
              width: 700,
            }}
          >
            <QueryLog language={language} logs={queryLogs}></QueryLog>
          </Paper>
        )}
        {connectInfo}
        <Stack direction="row" spacing={1} style={{ paddingLeft: '40px' }}>
          {loading && <CircularProgress size={progressBarSize} />}
          <Tooltip title="execute">
            <IconButton
              disabled={loading}
              onClick={() => {
                handleQuery(text);
              }}
              color="primary"
              aria-label="execute"
            >
              <PlayArrowIcon />
            </IconButton>
          </Tooltip>
          <Tooltip title="execute selected">
            <IconButton
              disabled={loading}
              onClick={() => {
                handleQuery(selectText);
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
              <RefreshIcon color={showSuggestReconn ? 'error' : 'success'} />
            </IconButton>
          </Tooltip>

          <Button
            onClick={() => {
              setShowQueryLog((prev) => !prev);
            }}
            color={showQueryLog ? 'error' : 'info'}
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
