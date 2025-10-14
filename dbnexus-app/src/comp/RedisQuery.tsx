'use strict';
import { useState, useRef, useEffect } from 'react';
import { editor } from 'monaco-editor';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';

import { SqlQuery } from './SqlQuery.tsx';
import { MakeIQueryConn } from './Common.ts';
import { appStore, AppStoreKey, DataSourceKind, ErrAndLoc } from './helper.ts';
import { Chip } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';
import { AppMarkDown } from './CommonComp2.tsx';

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

function RedisQuery(props: {
  makeConn: MakeIQueryConn;
  saveFavoriteSql: (name, sql: string) => void;
  favoriteName?: string;
  value?: string;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
  uuid: string;
}) {
  const { makeConn, saveFavoriteSql, favoriteName, value, getParseErr, uuid } =
    props;
  const [doc, setDoc] = useState('');
  const editorRef = useRef<editor.IStandaloneCodeEditor>(null);
  const [showDoc, setShowDoc] = useState(SaveShowDoc.instance.get());
  const [prevLine, setPrevLine] = useState('');
  useEffect(() => {
    SaveShowDoc.instance.save(showDoc);
  }, [showDoc]);

  useEffect(() => {
    const tryUpdateCommand = () => {
      if (!editorRef.current) {
        return;
      }
      const model = editorRef.current.getModel();
      let pos = editorRef.current.getPosition();
      if (!pos) {
        return;
      }
      const line = model.getLineContent(pos.lineNumber).trim();
      if (!line) {
        return;
      }
      if (line === prevLine) {
        // line not change no need to update
        return;
      }
      invoke<string>('get_redis_doc', { line })
        .then((e) => {
          setPrevLine(line);
          setDoc(e);
        })
        .catch(() => {
          // console.log('get redis doc failed', e);
        });
    };
    const int = setInterval(() => {
      tryUpdateCommand();
    }, 200);
    return () => {
      clearInterval(int);
    };
  }, [showDoc, prevLine]);

  return (
    <PanelGroup
      direction="horizontal"
      autoSaveId="redis-query"
      style={{
        height: '100%',
      }}
    >
      <Panel defaultSize={40}>
        <SqlQuery
          editorRef={editorRef}
          getParseErr={getParseErr}
          uuid={uuid}
          makeConn={makeConn}
          favoriteName={favoriteName}
          saveFavoriteSql={saveFavoriteSql}
          value={value}
          kind={DataSourceKind.Redis}
          extras={
            <>
              <Chip
                onClick={() => {
                  setShowDoc((e) => !e);
                }}
                color={showDoc ? 'success' : 'primary'}
                label={`${showDoc ? 'hidden' : 'show'} doc`}
              ></Chip>
            </>
          }
        ></SqlQuery>
      </Panel>

      {showDoc && (
        <PanelResizeHandle style={{ width: 4 }} className="resize-handle" />
      )}
      {showDoc && (
        <Panel
          collapsible
          collapsedSize={0}
          minSize={15}
          style={{
            display: 'flex',
            flexDirection: 'column',
          }}
        >
          <div
            style={{
              height: '100px',
              flexGrow: 1,
              flexShrink: 1,
              overflow: 'auto',
              padding: '0.3rem',
            }}
          >
            <AppMarkDown doc={doc} />
          </div>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { RedisQuery };
