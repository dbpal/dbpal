import { useCallback, useContext, useEffect, useState } from "react";

import { DefAndMetas } from "./Common";
import { DataSourceKind, TabContext } from "./helper";

import { AllKindChat } from "./Chat";
import { MetaValueView } from "./CommonComp";
import { CircularProgress, Stack, Tooltip, Typography } from "@mui/material";
import { useNotification, useQueryExtraUI } from "./store";
import { MetaValue, TriggerDetail } from "./bindings";

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
  const { showNotification } = useNotification();
  const handleReload = useCallback(
    (showOk) => {
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
              <MetaValueView values={detail.metas} />
            </Stack>
          }
          slotProps={{
            tooltip: {
              style: { width: "1550px" },
            },
          }}>
          <Typography color="info"> some meta </Typography>
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
        <AllKindChat uuid={uuid} kind={kind} value={detail.def} />
      </Stack>
    </>
  );
}

function triggerDetailsToMetas(x: TriggerDetail): MetaValue[] {
  const getEnableStr = (): string => {
    return x.enable.enable + "";
  };

  const ret: MetaValue[] = [
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

function TriggerDetailView(props: {
  reloadOP: () => Promise<TriggerDetail>;
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
export { DefAndMetasView, TriggerDetailView };
