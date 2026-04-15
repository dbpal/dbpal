import { useState, useEffect, useContext, useRef } from "react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

import { SqlQuery } from "./SqlQuery.tsx";
import {
  DataSourceKind,
  QueryIDEditorChatCtx,
  resizeHandlerSize,
  shrinkAble,
} from "./helper.ts";
import { Chip, Tooltip } from "@mui/material";
import { AppMarkDown } from "./CommonComp2.tsx";
import { useBoolSwitch } from "./store.tsx";
import { commands } from "./bindings.ts";
const showRedisDocKey = "redis-show-doc";
function RedisQuery(props: { favoriteName?: string; value?: string; uuid: string }) {
  const { favoriteName, value, uuid } = props;
  const [doc, setDoc] = useState("");
  const { getSwitch, setSwitch } = useBoolSwitch();
  const prevLine = useRef("");
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
      if (line === prevLine.current) {
        // line not change no need to update
        return;
      }
      commands
        .getRedisDoc(line)
        .then((e) => {
          prevLine.current = line;
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
  }, [showDoc, editor]);

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
