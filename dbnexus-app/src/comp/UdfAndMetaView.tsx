import { useCallback, useContext, useEffect, useRef, useState } from "react";

import { NotificationContext } from "./Notification";
import { Button, Stack, Tooltip } from "@mui/material";
import { DefAndMetas, NameAndQueryId, SetAutoCompletionName } from "./Common";
import { MetaValueView, PossibleParseError } from "./CommonComp";
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  getLanguageFromDataSourceKind,
  makeExecuteSql,
} from "./helper";
import { MultiQueryResp } from "./QueryResp";
import { editor } from "monaco-editor";
import { SimpleFlexEditor } from "./CommonComp2";
import types from "./ApiStruct";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  uuid: string;
  kind: DataSourceKind;
  showReload?: boolean;
}) {
  const { reloadOP, kind, showReload, uuid } = props;
  const executeOP = makeExecuteSql(uuid);
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas>({
    def: "",
    metas: [],
  });

  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [queryResp, setQueryResp] = useState<any[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [executeLoading, setExecuteLoading] = useState(false);
  const [parseErr] = useState<ErrAndLoc>(null);
  const handleReload = useCallback(
    showOk => {
      setLoading(true);
      reloadOP()
        .then(detail => {
          setDetail(detail);
          if (showOk) {
            showNotification("success", "reload ok");
          }
        })
        .catch(e => {
          showNotification("error", "fetch detail failed", e);
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [reloadOP, showNotification],
  );
  const handleExecute = () => {
    const sql = getEditorText(editor.current, showNotification);
    setExecuteLoading(true);
    executeOP(sql)
      .then(e => {
        setQueryResp([e]);
      })
      .catch(e => {
        showNotification("error", "execute failed", e);
      })
      .finally(() => {
        setExecuteLoading(false);
      });
  };
  useEffect(() => {
    handleReload(false);
  }, [handleReload]);
  return (
    <PanelGroup
      autoSaveId="udf-and-meta-view"
      direction="vertical"
      style={{ height: "100%" }}
    >
      <Panel style={{ display: "flex", flexDirection: "column" }}>
        <Stack height="100%">
          <Stack direction="row">
            {showReload && (
              <Button
                loading={loading}
                color="primary"
                onClick={() => {
                  handleReload(true);
                }}
              >
                reload
              </Button>
            )}
            <Tooltip title="execute the sql">
              <Button
                loading={executeLoading}
                color="success"
                style={{
                  minWidth: "3rem",
                }}
                onClick={() => {
                  handleExecute();
                }}
              >
                execute!
              </Button>
            </Tooltip>
          </Stack>
          {detail.def && (
            <SimpleFlexEditor
              language={getLanguageFromDataSourceKind(kind)}
              value={detail.def}
              handleEditor={ed => {
                editor.current = ed;
                SetAutoCompletionName.set(
                  ed.getModel().id,
                  new NameAndQueryId(uuid, kind),
                );
              }}
              barLeft=<Stack direction="row" flex={1}>
                {parseErr && (
                  <PossibleParseError parseErr={parseErr} editor={editor.current} />
                )}
              </Stack>
            />
          )}
        </Stack>
      </Panel>

      {(queryResp.length > 0 || (detail.metas && detail.metas.length > 0)) && (
        <>
          <PanelResizeHandle style={{ height: "4px" }} className="resize-handle" />
          <Panel
            minSize={10}
            collapsible
            collapsedSize={0}
            style={{
              display: "flex",
              flexDirection: "column",
            }}
          >
            {queryResp.length > 0 && (
              <div style={{ height: "100px", flex: 1 }}>
                <MultiQueryResp data={queryResp} />
              </div>
            )}
            {detail.metas && detail.metas.length > 0 && (
              <div
                style={{
                  height: "100px",
                  flex: 1,

                  display: "flex",
                  flexDirection: "column",
                }}
              >
                <MetaValueView values={detail.metas} />
              </div>
            )}
          </Panel>
        </>
      )}
    </PanelGroup>
  );
}

function triggerDetailsToMetas(x: types.TriggerDetail): types.MetaValue[] {
  const getEnableStr = (): string => {
    return x.enable.enable + "";
  };

  const getEnableDesc = (): string => {
    if (x.enable.kind == "postgres") {
      return `Controls in which session_replication_role modes the trigger fires. O = trigger fires in
      “origin” and “local” modes, D = trigger is disabled, R = trigger fires in “replica” mode, A
      = trigger fires always.`;
    }
    return "";
  };
  const ret: types.MetaValue[] = [
    // todo timing can be object
    { name: "timing", value: x.timing as string, desc: "" },
    { name: "event", value: x.event, desc: "" },
  ];
  if (x.enable) {
    ret.push({
      name: "enable",
      value: getEnableStr(),
      desc: getEnableDesc(),
    });
  }
  return ret;
}

function TriggerDetail(props: {
  reloadOP: () => Promise<types.TriggerDetail>;
  uuid: string;
  kind: DataSourceKind;
}) {
  const { reloadOP, uuid, kind } = props;
  return (
    <DefAndMetasView
      kind={kind}
      reloadOP={async () => {
        return reloadOP().then(detail => {
          return {
            def: detail.sql,
            metas: [...triggerDetailsToMetas(detail), ...detail.metas],
            detail,
          };
        });
      }}
      uuid={uuid}
    />
  );
}
export { DefAndMetasView, TriggerDetail };
