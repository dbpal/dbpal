import { useState, useRef, useEffect } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Position } from 'monaco-editor';

import { SqlQuery } from './SqlQuery.tsx';
import { fetch } from '@tauri-apps/plugin-http';
import { MakeIQueryConn } from './Common.tsx';
import { DataSourceKind, SaveSizesToStore } from './helper.ts';
function RedisQuery(props: {
  makeConn: MakeIQueryConn;
  language;
  saveFavoriteSql: (name, sql: string) => void;
  favoriteName?: string;
  value?: string;
}) {
  const {
    language,
    makeConn,

    saveFavoriteSql,
    favoriteName,
    value,
  } = props;
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
    return urlPrefix + command + '/' + '#examples';
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
  const validURL = async (url) => {
    let x = await fetch(url);
    return x.ok;
  };
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
    cmd = cmd.toLowerCase();
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
  const [size, setSize] = useState<string | number>('40vw');
  const sizeKey = 'reids-query-size';
  useEffect(() => {
    SaveSizesToStore.getSize(sizeKey).then((e) => {
      if (e && e.width) {
        setSize(e.width);
      }
    });
  }, []);

  return (
    //@ts-ignore
    <SplitPane
      split="vertical"
      minSize={300}
      defaultSize={size}
      step={5}
      onDragStarted={() => {
        setIsDragging(true);
      }} // 拖动开始
      //@ts-ignore
      onDragFinished={(_: number): void => {
        setIsDragging(false);
      }} // 拖动结束
      style={{ position: 'relative' }}
      onChange={(newsize) => {
        SaveSizesToStore.saveSize(sizeKey, { width: newsize });
      }}
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
        language={language}
        makeConn={makeConn}
        favoriteName={favoriteName}
        saveFavoriteSql={saveFavoriteSql}
        value={value}
        kind={DataSourceKind.Redis}
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

export { RedisQuery };
