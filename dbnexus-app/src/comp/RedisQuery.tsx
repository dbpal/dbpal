import { useState, useRef, useEffect, useContext } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import Editor, { loader } from '@monaco-editor/react';
import { Position, editor, languages } from 'monaco-editor';
import { CancellationToken } from 'monaco-editor';
import { IconButton, Stack, Tooltip } from '@mui/material';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import PlayCircleIcon from '@mui/icons-material/PlayCircle';
import { NotificationContext } from './Notification';
import { MultiQueryResp } from './QueryResp';
import { ClassNames } from '@emotion/react';

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
        root: [
          [
            /\b(APPEND|ASKING|AUTH|BGREWRITEAOF|BGSAVE|BITCOUNT|BITFIELD|BITOP|BITPOS|BLPOP|BRPOP|BRPOPLPUSH|CLIENT|CLUSTER|CONFIG|DBSIZE|DEBUG|DECR|DECRBY|DEL|DISCARD|DUMP|ECHO|EVAL|EVALSHA|EXEC|EXISTS|EXPIRE|EXPIREAT|FLUSHALL|FLUSHDB|GEOADD|GEODIST|GEOHASH|GEOPOS|GEORADIUS|GEORADIUSBYMEMBER|GET|GETBIT|GETRANGE|GETSET|HDEL|HEXISTS|HGET|HGETALL|HINCRBY|HINCRBYFLOAT|HKEYS|HLEN|HMGET|HMSET|HSCAN|HSET|HSETNX|HSTRLEN|HVALS|INCR|INCRBY|INCRBYFLOAT|INFO|KEYS|LASTSAVE|LINDEX|LINSERT|LLEN|LPOP|LPUSH|LPUSHX|LRANGE|LREM|LSET|LTRIM|MGET|MIGRATE|MOVE|MSET|MSETNX|MULTI|OBJECT|PERSIST|PEXPIRE|PEXPIREAT|PFADD|PFCOUNT|PFMERGE|PING|PSETEX|PTTL|PUBLISH|RANDOMKEY|READONLY|READWRITE|RENAME|RENAMENX|RESTORE|RESTORE_ASKING|ROLE|RPOP|RPOPLPUSH|RPUSH|RPUSHX|SADD|SAVE|SCAN|SCARD|SCRIPT|SDIFF|SDIFFSTORE|SET|SETBIT|SETEX|SETNX|SETRANGE|SHUTDOWN|SINTER|SINTERSTORE|SISMEMBER|SLAVEOF|SLOWLOG|SMEMBERS|SMOVE|SORT|SPOP|SRANDMEMBER|SREM|SSCAN|STRLEN|SUNION|SUNIONSTORE|SYNC|TIME|TOUCH|TTL|TYPE|UNLINK|ZADD|ZCARD|ZCOUNT|ZINCRBY|ZINTERSTORE|ZLEXCOUNT|ZRANGE|ZRANGEBYLEX|ZRANGEBYSCORE|ZRANK|ZREM|ZREMRANGEBYLEX|ZREMRANGEBYRANK|ZREMRANGEBYSCORE|ZREVRANGE|ZREVRANGEBYLEX|ZREVRANGEBYSCORE|ZREVRANK|ZSCAN|ZSCORE|ZUNIONSTORE)\b/,
            'keyword',
          ],
        ],
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
  executeOP: (code: string) => Promise<any[]>;
  validURL: (string) => Promise<boolean>;
}) {
  const { executeOP, validURL } = props;
  const [command, setCommand] = useState('get');
  const [isDragging, setIsDragging] = useState(false);
  const iframeRef = useRef(null);
  const [resp, setResp] = useState([]);

  // 禁用 iframe 的鼠标事件
  const disableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'none';
    }
  };
  const urlPrefix = 'https://redis.io/docs/latest/commands/';

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

  return (
    <div style={{ display: 'flex' }}>
      {/* // @ts-ignore */}
      <SplitPane
        split='vertical'
        minSize={50}
        defaultSize={1000}
        step={5}
        onDragStarted={() => setIsDragging(true)} // 拖动开始
        onDragFinished={() => setIsDragging(false)} // 拖动结束
      >
        <div style={{ width: '100%' }}>
          {/* // @ts-ignore */}
          <SplitPane split='horizontal' defaultSize={500}>
            <div style={{ width: '100%' }}>
              <RedisEditor
                executeOP={(text) => {
                  executeOP(text)
                    .then((resp) => {
                      setResp(resp);
                    })
                    .catch((e) => {
                      showNotification('error', 'execute error', e);
                    });
                }}
                handelTextAndPositionChange={tryUpdateCommand}
              ></RedisEditor>
            </div>
            <div style={{ width: '100%' }}>
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
    </div>
  );
}

function RedisEditor(props: {
  executeOP: (code: string) => void;
  handelTextAndPositionChange: (text: string, pos: Position) => void;
}) {
  const [text, setText] = useState('');
  const [selectText, setSelectText] = useState('');
  const [pos, setPos] = useState<Position | null>(null);
  let { executeOP, handelTextAndPositionChange } = props;

  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    if (!pos) {
      return;
    }
    if (!text) {
      return;
    }
    handelTextAndPositionChange(text, pos);
  }, [text, pos]);
  const handleEditorDidMount = (editor: editor.IStandaloneCodeEditor) => {
    editor.onDidChangeModelContent((_event) => {
      setText(editor.getValue());
      console.log('new text:', text);
    });

    editor.onDidChangeCursorPosition((_event) => {
      setPos(editor.getPosition());
      console.log('new pos:', pos);
    });
    editor.onDidChangeCursorSelection((_event) => {
      const selection = editor.getSelection();
      setSelectText(editor.getModel().getValueInRange(selection));
      console.log('new select text:', selectText);
    });
  };
  const handleExecute = (text: string) => {
    if (!text) {
      showNotification('error', "can't execute empty statement.");
      return;
    }
    executeOP(text);
  };

  return (
    <Stack style={{ width: '100%', height: '500px' }}>
      <Stack direction='row' spacing={0}>
        <Tooltip title='execute'>
          <IconButton
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
            color='primary'
            aria-label='execute'
          >
            <PlayCircleIcon />
          </IconButton>
        </Tooltip>
      </Stack>
      <Editor onMount={handleEditorDidMount} language='redis'></Editor>
    </Stack>
  );
}

export { RedisQuery, initRedisLanguage };
