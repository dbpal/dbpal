import { useCallback, useContext, useEffect, useState } from "react";

import { TabContext } from "./helper";

import { AllKindChat } from "./Chat";
import { MetaValueView } from "./CommonComp";
import { Chip, CircularProgress, Stack, Tooltip } from "@mui/material";
import { useNotification, useQueryExtraUI } from "./store";
import { DefAndMetas, MetaValue, TriggerDetail } from "./bindings";

function DefAndMetasView(props: { reloadOP: () => Promise<DefAndMetas> }) {
  const { reloadOP } = props;
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas>({
    def: "",
    metas: [],
  });
  const { showNotification } = useNotification();
  const handleReload = useCallback(
    (showOk: boolean) => {
      setLoading(true);
      reloadOP()
        .then((detail) => {
          setDetail(detail);
          if (showOk) {
            showNotification({ severity: "success", title: "reload ok" });
          }
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "fetch detail failed",
            message: e,
          });
        })
        .finally(() => {
          setLoading(false);
        });
    },
    [reloadOP, showNotification],
  );
  const tabCtx = useContext(TabContext);
  const { setExtra, removeExtra } = useQueryExtraUI();
  useEffect(() => {
    setExtra(
      tabCtx.id,
      detail.metas.length > 0 ? (
        <Tooltip
          title={
            <Stack style={{ width: "100%", height: "40vh" }}>
              <MetaValueView values={detail.metas} showToolbar />
            </Stack>
          }
          slotProps={{
            tooltip: {
              style: { width: "1550px" },
            },
          }}>
          <Chip label="some meta " />
        </Tooltip>
      ) : null,
    );
  }, [detail.metas, setExtra, tabCtx.id]);
  useEffect(() => {
    return () => {
      removeExtra(tabCtx.id);
    };
  }, [removeExtra, tabCtx.id]);

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  return (
    <>
      <Stack sx={{ height: "100%" }}>
        {loading && <CircularProgress />}
        <AllKindChat value={detail.def} />
      </Stack>
    </>
  );
}

function triggerDetailsToMetas(x: TriggerDetail): MetaValue[] {
  const getEnableStr = (): string => {
    return x.enable.enable + "";
  };

  const ret: MetaValue[] = [
    {
      name: "timing",
      value: (function () {
        if (typeof x.timing == "object") {
          return x.timing.String;
        } else {
          return x.timing;
        }
      })(),
    },
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

function TriggerDetailView(props: { reloadOP: () => Promise<TriggerDetail> }) {
  const { reloadOP } = props;
  return (
    <DefAndMetasView
      reloadOP={async () => {
        return reloadOP().then((detail) => {
          return {
            def: detail.sql,
            metas: [...triggerDetailsToMetas(detail), ...detail.metas],
            detail,
          };
        });
      }}
    />
  );
}
export { DefAndMetasView, TriggerDetailView };
