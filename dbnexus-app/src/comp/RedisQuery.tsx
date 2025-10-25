'use strict';
import { useState, useRef, useEffect } from 'react';
import { editor } from 'monaco-editor';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';

import { SqlQuery } from './SqlQuery.tsx';
import { MakeIQueryConn } from './Common.ts';
import { DataSourceKind, ErrAndLoc, shrinkAble } from './helper.ts';
import { Chip, Tooltip } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';
import { AppMarkDown } from './CommonComp2.tsx';
import { message } from '@tauri-apps/plugin-dialog';

class SaveShowDoc {
  key = 'dbpal-redis-show-doc';

  save(x: boolean) {
    localStorage.setItem(this.key, x ? '1' : '0');
  }

  get(): boolean {
    return localStorage.getItem(this.key) == '1';
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
      const pos = editorRef.current.getPosition();
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
        position: 'relative',
      }}
    >
      <Panel defaultSize={40} minSize={4}>
        <SqlQuery
          hasPositionRelative
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
              <Tooltip title="toggle show redis document">
                <Chip
                  onClick={() => {
                    setShowDoc((e) => !e);
                  }}
                  style={shrinkAble}
                  color={showDoc ? 'success' : 'primary'}
                  label={`${showDoc ? 'hidden' : 'show'} doc`}
                ></Chip>
              </Tooltip>
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
              padding: '0.4rem',
            }}
          >
            <AppMarkDown
              handlePathUrl={(href) => {
                message(
                  `can't goto '${href}',we are sorry about this document implementation for redis is limited.`,
                  { kind: 'error', okLabel: 'dismiss' }
                );
              }}
              doc={doc}
            />
          </div>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { RedisQuery };
