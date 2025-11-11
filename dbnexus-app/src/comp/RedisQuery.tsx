"use strict";
import { useState, useRef, useEffect } from "react";
import { editor } from "monaco-editor";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

import { SqlQuery } from "./SqlQuery.tsx";
import { DataSourceKind, shrinkAble } from "./helper.ts";
import { Chip, Tooltip } from "@mui/material";
import { invoke } from "@tauri-apps/api/core";
import { AppMarkDown } from "./CommonComp2.tsx";

class SaveShowDoc {
  key = "dbpal-redis-show-doc";

  save(x: boolean) {
    localStorage.setItem(this.key, x ? "1" : "0");
  }

  get(): boolean {
    return localStorage.getItem(this.key) == "1";
  }
  static instance = new SaveShowDoc();
}

function RedisQuery(props: {
  favoriteName?: string;
  value?: string;
  uuid: string;
  handleEditor: (x: editor.IStandaloneCodeEditor) => void;
}) {
  const { favoriteName, value, uuid, handleEditor } = props;
  const [doc, setDoc] = useState("");
  const editorRef = useRef<editor.IStandaloneCodeEditor>(null);
  const [showDoc, setShowDoc] = useState(SaveShowDoc.instance.get());
  const [prevLine, setPrevLine] = useState("");
  useEffect(() => {
    SaveShowDoc.instance.save(showDoc);
  }, [showDoc]);

  useEffect(() => {
    const tryUpdateCommand = () => {
      if (!showDoc) {
        return;
      }
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
      invoke<string>("get_redis_doc", { line })
        .then(e => {
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
        height: "100%",
      }}
    >
      <Panel defaultSize={40} minSize={4}>
        <SqlQuery
          handleEditor={e => {
            handleEditor(e);
            editorRef.current = e;
          }}
          uuid={uuid}
          favoriteName={favoriteName}
          value={value}
          kind={DataSourceKind.Redis}
          extras={
            <>
              <Tooltip title="toggle show redis document">
                <Chip
                  onClick={() => {
                    setShowDoc(e => !e);
                  }}
                  style={shrinkAble}
                  color={showDoc ? "success" : "primary"}
                  label={`${showDoc ? "hidden" : "show"} doc`}
                ></Chip>
              </Tooltip>
            </>
          }
        ></SqlQuery>
      </Panel>

      {showDoc && <PanelResizeHandle style={{ width: 4 }} className="resize-handle" />}
      {showDoc && (
        <Panel
          collapsible
          collapsedSize={0}
          minSize={15}
          style={{
            display: "flex",
            flexDirection: "column",
          }}
        >
          <div
            style={{
              height: "100px",
              flexGrow: 1,
              flexShrink: 1,
              overflow: "auto",
              padding: "0.4rem",
            }}
          >
            <AppMarkDown
              children={doc}
              placeHolder="when typing markdown document will show atomically"
            />
          </div>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { RedisQuery };
