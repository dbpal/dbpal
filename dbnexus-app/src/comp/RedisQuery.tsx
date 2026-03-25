import { useState, useRef, useEffect } from "react";
import { editor } from "monaco-editor";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

import { SqlQuery } from "./SqlQuery.tsx";
import { DataSourceKind, resizeHandlerSize, shrinkAble } from "./helper.ts";
import { Chip, Tooltip } from "@mui/material";
import { invoke } from "@tauri-apps/api/core";
import { AppMarkDown } from "./CommonComp2.tsx";
import { useBoolSwitch } from "./store.ts";

function RedisQuery(props: {
  favoriteName?: string;
  value?: string;
  uuid: string;
  handleEditor: (x: editor.IStandaloneCodeEditor) => void;
}) {
  const { favoriteName, value, uuid, handleEditor } = props;
  const [doc, setDoc] = useState("");
  const editorRef = useRef<editor.IStandaloneCodeEditor>(null);
  const { switches, setSwitch } = useBoolSwitch((s) => s);

  const [prevLine, setPrevLine] = useState("");

  const switchKey = "redis-show-doc";

  const showDoc = switches[switchKey];

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
    }, 1000);
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
          handleEditor={(e) => {
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
                    setSwitch(switchKey, !showDoc);
                  }}
                  style={shrinkAble}
                  color={showDoc ? "success" : "primary"}
                  label={`${showDoc ? "hidden" : "show"} doc`}
                />
              </Tooltip>
            </>
          }
        />
      </Panel>

      {showDoc && (
        <PanelResizeHandle
          style={{ width: resizeHandlerSize }}
          className="resize-handle"
        />
      )}
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
            <AppMarkDown placeHolder="when typing markdown document will show atomically">
              {doc}
            </AppMarkDown>
          </div>
        </Panel>
      )}
    </PanelGroup>
  );
}

export { RedisQuery };
