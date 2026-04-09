import { useState, useEffect, useContext } from "react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

import { SqlQuery } from "./SqlQuery.tsx";
import {
  DataSourceKind,
  QueryIDEditorChatCtx,
  resizeHandlerSize,
  shrinkAble,
} from "./helper.ts";
import { Chip, Tooltip } from "@mui/material";
import { invoke } from "@tauri-apps/api/core";
import { AppMarkDown } from "./CommonComp2.tsx";
import { useBoolSwitch } from "./store.tsx";
const showRedisDocKey = "redis-show-doc";
function RedisQuery(props: { favoriteName?: string; value?: string; uuid: string }) {
  const { favoriteName, value, uuid } = props;
  const [doc, setDoc] = useState("");
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [prevLine, setPrevLine] = useState("");
  const showDoc = getSwitch(showRedisDocKey);

  const { editor } = useContext(QueryIDEditorChatCtx);
  useEffect(() => {
    const tryUpdateCommand = () => {
      if (!showDoc) {
        return;
      }
      if (!editor) {
        return;
      }
      const model = editor.getModel();
      const pos = editor.getPosition();
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
  }, [showDoc, prevLine, editor]);

  return (
    <PanelGroup
      direction="horizontal"
      autoSaveId="redis-query"
      style={{
        height: "100%",
      }}>
      <Panel defaultSize={40} minSize={4}>
        <SqlQuery
          uuid={uuid}
          favoriteName={favoriteName}
          value={value}
          kind={DataSourceKind.Redis}
          extras={
            <>
              <Tooltip title="toggle show redis document">
                <Chip
                  onClick={() => {
                    setSwitch(showRedisDocKey, !showDoc);
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
          }}>
          <div
            style={{
              height: "100px",
              flexGrow: 1,
              flexShrink: 1,
              overflow: "auto",
            }}>
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
