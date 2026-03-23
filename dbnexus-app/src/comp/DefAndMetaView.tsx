import { useCallback, useContext, useEffect, useState } from "react";

import { NotificationContext } from "./Notification";
import { DefAndMetas } from "./Common";
import { DataSourceKind, resizeHandlerSize } from "./helper";
import types from "./ApiStruct";
import { AllKindChat } from "./Chat";
import { MetaValueView } from "./CommonComp";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { CircularProgress } from "@mui/material";

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  uuid: string;
  kind: DataSourceKind;
}) {
  const { reloadOP, kind, uuid } = props;
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas>({
    def: "",
    metas: [],
  });
  const { showNotification } = useContext(NotificationContext);
  const handleReload = useCallback(
    (showOk) => {
      setLoading(true);
      reloadOP()
        .then((detail) => {
          setDetail(detail);
          if (showOk) {
            showNotification("success", "reload ok");
          }
        })
        .catch((e) => {
          showNotification("error", "fetch detail failed", e);
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [reloadOP, showNotification],
  );

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);
  return (
    <PanelGroup style={{ height: "100%" }} direction="vertical" autoSaveId="udf-view">
      <Panel>
        {loading && <CircularProgress />}
        <AllKindChat uuid={uuid} kind={kind} value={detail.def} />
      </Panel>
      {detail.metas.length > 0 && (
        <>
          <PanelResizeHandle
            style={{ height: resizeHandlerSize }}
            className="resize-handle"
          />
          <Panel collapsible collapsedSize={0} minSize={10}>
            <div
              style={{
                height: "100%",
                display: "flex",
                flexDirection: "column",
              }}
            >
              <MetaValueView values={detail.metas} />
            </div>
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

  const ret: types.MetaValue[] = [
    // todo timing can be object
    { name: "timing", value: x.timing as string },
    { name: "event", value: x.event },
  ];
  if (x.enable) {
    ret.push({
      name: "enable",
      value: getEnableStr(),
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
        return reloadOP().then((detail) => {
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
