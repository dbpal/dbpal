import { useState, useRef, useEffect, useCallback } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { editor } from 'monaco-editor';

import { SqlQuery } from './SqlQuery.tsx';
import { MakeIQueryConn } from './Common.ts';
import {
  appStore,
  AppStoreKey,
  DataSourceKind,
  ErrAndLoc,
  SaveDomSizes,
  validURL,
} from './helper.ts';
import { Chip } from '@mui/material';

class SaveShowDoc {
  showDocOld = true;
  save(x: boolean) {
    this.showDocOld = x;
    appStore.set(AppStoreKey.RedisShowDoc, x);
  }

  constructor() {
    appStore.get<boolean>(AppStoreKey.RedisShowDoc).then((x) => {
      if (typeof x === 'boolean') {
        this.showDocOld = x;
      }
    });
  }
  get(): boolean {
    return this.showDocOld;
  }
  static instance = new SaveShowDoc();
}

const urlPrefix = 'https://redis.io/docs/latest/commands/';
const getURL = (command: string) => {
  return urlPrefix + command + '/' + '#examples';
};

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
  const editorRef = useRef<editor.IStandaloneCodeEditor>(null);

  // 禁用 iframe 的鼠标事件
  const disableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'none';
    }
  };
  const [showDoc, setShowDoc] = useState(SaveShowDoc.instance.get());

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

  const tryUpdateCommand = useCallback(() => {
    if (!showDoc) {
      return;
    }
    if (!editorRef.current) {
      return;
    }
    const text = editorRef.current.getValue();
    const pos = editorRef.current.getPosition();
    const lineArray = text.split('\n');
    const line = pos.lineNumber - 1;
    if (line >= lineArray.length) {
      return;
    }
    let cmds = lineArray[line].split(' ');
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
    if (cmd == command) {
      return; // no need to change
    }
    const url = getURL(cmd);
    validURL(url)
      .then((e) => {
        if (e) {
          setCommand(cmd);
        }
      })
      .catch(() => {});
  }, [showDoc]);

  useEffect(() => {
    const int = setInterval(() => {
      tryUpdateCommand();
    }, 1000);
    return () => {
      clearInterval(int);
    };
  }, [tryUpdateCommand]);
  const [size, setSize] = useState<string | number>('40vw');

  const sizeKey = 'reids-query-size';
  useEffect(() => {
    SaveDomSizes.getSize(sizeKey).then((e) => {
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
    SaveShowDoc.instance.save(showDoc);
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
      onDragFinished={(_: number): void => {
        setIsDragging(false);
      }} // 拖动结束
      onChange={(newSize) => {
        SaveDomSizes.saveSize(sizeKey, { width: newSize });
      }}
      style={{ width: '99%' }}
    >
      <SqlQuery
        editorRef={editorRef}
        getParseErr={getParseErr}
        uuid={language}
        makeConn={makeConn}
        favoriteName={favoriteName}
        saveFavoriteSql={saveFavoriteSql}
        value={value}
        kind={DataSourceKind.Redis}
        buttons={
          <>
            <Chip
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
