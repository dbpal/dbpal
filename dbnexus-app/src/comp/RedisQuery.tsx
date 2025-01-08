import { useState, useRef, useEffect } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Position, editor, languages } from 'monaco-editor';
import { CancellationToken } from 'monaco-editor';
import types from './ApiStruct';
import { redisKeywords } from './keywords.ts';
import { SqlQuery } from './SqlQuery.tsx';
import { loader } from '@monaco-editor/react';

function RedisQuery(props: {
  queryOP: (code: string) => Promise<any[]>;
  validURL: (string) => Promise<boolean>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
}) {
  const { queryOP, validURL, loadQueryLog } = props;
  const [command, setCommand] = useState('get');
  const [isDragging, setIsDragging] = useState(false);
  const iframeRef = useRef(null);

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
    //@ts-ignore
    <SplitPane
      split="vertical"
      minSize={50}
      defaultSize={1000}
      step={5}
      onDragStarted={() => {
        setIsDragging(true);
      }} // 拖动开始
      //@ts-ignore
      onDragFinished={(_: number): void => {
        setIsDragging(false);
      }} // 拖动结束
      style={{ position: 'relative' }}
    >
      <SqlQuery
        handelTextAndPositionChange={(text, pos) => {
          if (!pos) {
            return;
          }
          if (!text) {
            return;
          }
          tryUpdateCommand(text, pos);
        }}
        language="redis"
        queryOP={queryOP}
        loadQueryLog={loadQueryLog}
      ></SqlQuery>

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
          title="Full Size Iframe"
        />
      </div>
    </SplitPane>
  );
}

export { RedisQuery, initLanguageXXX };
