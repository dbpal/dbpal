import { useState, useRef, useEffect } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Position } from 'monaco-editor';

import { SqlQuery } from './SqlQuery.tsx';
import { fetch } from '@tauri-apps/plugin-http';
import { MakeIQueryConn } from './Common.tsx';
import { DataSourceKind, ErrAndLoc, SaveSizesToStore } from './helper.ts';
import { Chip } from '@mui/material';

let showDocOld = true;

function RedisQuery(props: {
  makeConn: MakeIQueryConn;
  language;
  saveFavoriteSql: (name, sql: string) => void;
  favoriteName?: string;
  value?: string;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
}) {
  const {
    language,
    makeConn,
    saveFavoriteSql,
    favoriteName,
    value,
    getParseErr,
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
  const [showDoc, setShowDoc] = useState(showDocOld);
  const sizeKey = 'reids-query-size';
  useEffect(() => {
    SaveSizesToStore.getSize(sizeKey).then((e) => {
      if (e && e.width && showDoc) {
        setSize(e.width + 'px');
      }
    });
  }, []);
  useEffect(() => {
    if (showDoc) {
      setSize('40vw');
    } else {
      setSize('100%');
    }
    showDocOld = showDoc;
  }, [showDoc]);

  return (
    //@ts-ignore
    <SplitPane
      split="vertical"
      minSize={200}
      size={size}
      step={5}
      onDragStarted={() => {
        setIsDragging(true);
      }} // 拖动开始
      //@ts-ignore
      onDragFinished={(_: number): void => {
        setIsDragging(false);
      }} // 拖动结束
      onChange={(newsize) => {
        SaveSizesToStore.saveSize(sizeKey, { width: newsize });
      }}
      style={{ width: '99%' }}
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
        getParseErr={getParseErr}
        language={language}
        makeConn={makeConn}
        favoriteName={favoriteName}
        saveFavoriteSql={saveFavoriteSql}
        value={value}
        kind={DataSourceKind.Redis}
        buttons={
          <>
            <Chip
              size="small"
              onClick={() => {
                setShowDoc((e) => !e);
              }}
              color="primary"
              label={`${showDoc ? 'hidden' : 'show'} doc`}
            ></Chip>
          </>
        }
      ></SqlQuery>

      <div style={{ height: '100vh' }}>
        {showDoc && (
          <iframe
            hidden={!showDoc}
            ref={iframeRef} // 绑定 iframe 引用
            src={getURL(command)}
            style={{
              width: '100%',
              height: '100%',
              border: 'none', // 去掉边框
            }}
            title="Full Size Iframe"
          />
        )}
      </div>
    </SplitPane>
  );
}

export { RedisQuery };
