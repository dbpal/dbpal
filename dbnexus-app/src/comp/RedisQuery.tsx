import { useState, useRef, useEffect, useContext } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import Editor, { loader } from '@monaco-editor/react';
import { Position, editor, languages } from 'monaco-editor';
import { CancellationToken } from 'monaco-editor';
import {
  Button,
  CircularProgress,
  IconButton,
  Stack,
  Tooltip,
} from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import PlayCircleIcon from '@mui/icons-material/PlayCircle';
import { NotificationContext } from './Notification';
import { MultiQueryResp } from './QueryResp';
import { EditorBar } from './common';
import { QueryLog } from './QueryLog';
import types from './ApiStruct';
import { redisKeywords } from './keywords.ts';
import CloseIcon from '@mui/icons-material/Close';

function initRedisLanguage(
  autoCompletion: (
    text: string,
    pos: Position
  ) => Promise<languages.CompletionList>
) {
  loader.init().then((monaco) => {
    monaco.languages.register({ id: 'redis' });
    monaco.languages.setMonarchTokensProvider('monaco', {
      ignoreCase: true,
      tokenizer: {
        root: [[redisKeywords, 'keyword']],
      },
    });

    monaco.languages.registerCompletionItemProvider('redis', {
      provideCompletionItems: (
        model: editor.ITextModel,
        position: Position,
        _context: any,
        _token: CancellationToken
      ) => {
        let text = model.getValue();
        return autoCompletion(text, position);
      },
    });
  });
}

function RedisQuery(props: {
  queryOP: (code: string) => Promise<any[]>;
  validURL: (string) => Promise<boolean>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
}) {
  const { queryOP, validURL, loadQueryLog } = props;
  const [command, setCommand] = useState('get');
  const [isDragging, setIsDragging] = useState(false);
  const iframeRef = useRef(null);
  const [resp, setResp] = useState([]);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [queryLogs, setQueryLogs] = useState([]);
  const [loading, setLoading] = useState(false);
  const [text, setText] = useState('');
  const [selectText, setSelectText] = useState('');

  // 禁用 iframe 的鼠标事件
  const disableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'none';
    }
  };

  const urlPrefix = 'https://redis.io/docs/latest/commands/';

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

  const getURL = (command: string) => {
    return urlPrefix + command + '/';
  };

  const { showNotification } = useContext(NotificationContext);
  // 启用 iframe 的鼠标事件
  const enableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'auto';
    }
  };

  useEffect(() => {
    if (isDragging) {
      disableIframe();
    } else {
      enableIframe();
    }
  }, [isDragging]);

  const tryUpdateCommand = (text: string, pos: Position) => {
    const linesArray = text.split('\n');
    const line = pos.lineNumber - 1;
    if (line >= linesArray.length) {
      return;
    }
    let cmds = linesArray[line].split(' ');
    let cmd = '';
    for (let i = 0; i < cmds.length; i++) {
      let c = cmds[i];
      if (c) {
        cmd = c;
        break;
      }
    }
    if (!cmd) {
      return;
    }
    let url = getURL(cmd);

    validURL(url)
      .then((e) => {
        if (e) {
          setCommand(cmd);
        }
      })
      .catch((e) => {
        console.log('not valid url error:', e);
      });
  };

  const handleExecute = (text: string) => {
    if (!text.trim()) {
      showNotification('error', `can't execute empty statement.`);
      return;
    }
    setLoading(true);
    queryOP(text)
      .then((resp) => {
        setLoading(false);
        setResp(resp);
      })
      .catch((e) => {
        showNotification('error', 'execute failed', e);
        setLoading(false);
      });
  };

  return (
    <>
      {/* // @ts-ignore */}
      <SplitPane
        split='vertical'
        minSize={50}
        defaultSize={1000}
        step={5}
        onDragStarted={() => setIsDragging(true)} // 拖动开始
        onDragFinished={(_) => setIsDragging(false)} // 拖动结束
        style={{ position: 'relative' }}
      >
        <div>
          {showQueryLog && (
            <div
              style={{
                position: 'absolute',
                opacity: 1,
                right: 0,
                top: 0,
                display: showQueryLog ? 'block' : 'none',
                zIndex: 14,
                width: 700,
              }}
            >
              <QueryLog language='redis' logs={queryLogs}></QueryLog>
            </div>
          )}

          {/* // @ts-ignore */}
          <SplitPane split='horizontal' defaultSize={500}>
            <div
              style={{
                width: '100%',
                display: 'flex',
                flexDirection: 'column',
              }}
            >
              <Stack
                style={{ paddingLeft: '50px' }}
                direction='row'
                spacing={0}
              >
                {loading && <CircularProgress />}
                <Tooltip title='execute'>
                  <IconButton
                    disabled={loading}
                    onClick={() => {
                      handleExecute(text);
                    }}
                    color='primary'
                    aria-label='execute'
                  >
                    <PlayArrowIcon />
                  </IconButton>
                </Tooltip>
                <Tooltip title='execute selected'>
                  <IconButton
                    onClick={() => {
                      handleExecute(selectText);
                    }}
                    disabled={loading}
                    color='primary'
                    aria-label='execute'
                  >
                    <PlayCircleIcon />
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

              <RedisEditor
                handleText={(e) => {
                  setText(e);
                }}
                handleSelectText={(e) => {
                  setSelectText(e);
                }}
                handelTextAndPositionChange={tryUpdateCommand}
              ></RedisEditor>
            </div>
            <div>
              <MultiQueryResp data={resp}></MultiQueryResp>
            </div>
          </SplitPane>
        </div>

        <div
          style={{
            height: '100vh',
            position: 'relative',
          }}
        >
          <iframe
            ref={iframeRef} // 绑定 iframe 引用
            src={getURL(command)}
            style={{
              width: '100%',
              height: '100%',
              border: 'none', // 去掉边框
              position: 'absolute',
              top: 0,
              left: 0,
              padding: 0,
              margin: 0,
            }}
            title='Full Size Iframe'
          />
        </div>
      </SplitPane>
    </>
  );
}

function RedisEditor(props: {
  handelTextAndPositionChange: (text: string, pos: Position) => void;
  handleText: (text: string) => void;
  handleSelectText: (text: string) => void;
}) {
  let { handelTextAndPositionChange, handleText, handleSelectText } = props;

  const [pos, setPos] = useState<Position | null>(null);

  const handleEditorDidMount = (editor: editor.IStandaloneCodeEditor) => {
    editor.onDidChangeModelContent((_event) => {
      let text = editor.getValue();
      handelTextAndPositionChange(text, pos);
      handleText(text);
    });
    editor.onDidChangeCursorPosition((_event) => {
      let pos = editor.getPosition();
      setPos(pos);
      handelTextAndPositionChange(editor.getValue(), pos);
    });

    editor.onDidChangeCursorSelection((_event) => {
      const selection = editor.getSelection();
      handleSelectText(editor.getModel().getValueInRange(selection));
    });
  };

  return (
    <div
      style={{
        width: '100%',
        height: '100px',
        display: 'flex',
        flexDirection: 'column',
        flexGrow: 1,
        flexShrink: 1,
        minHeight: 0,
      }}
    >
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          minHeight: 0,
          height: '100px',
        }}
      >
        <Editor
          onMount={handleEditorDidMount}
          height='100%'
          language='redis'
        ></Editor>
      </div>
      <EditorBar style={{ paddingBottom: '10px' }} pos={pos}></EditorBar>
    </div>
  );
}

export { RedisQuery, initRedisLanguage };
