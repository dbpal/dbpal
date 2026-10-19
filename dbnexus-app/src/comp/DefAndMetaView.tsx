import { useCallback, useEffect, useState } from "react";

import { MetaValueView } from "./CommonComp";
import { CircularProgress, Stack } from "@mui/material";
import { useNotification } from "./store";
import { DefAndMetas, MetaValue, TriggerDetail } from "./bindings";
import { AppCode } from "./CommonComp2";

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

  useEffect(() => {
    handleReload(false);
  }, [handleReload]);

  return (
    <>
      <Stack sx={{ height: "100%" }}>
        {loading && <CircularProgress />}
        <AppCode codeBlock={detail.def} language="sql" />
        {detail.metas && detail.metas.length > 0 && (
          <MetaValueView values={detail.metas} />
        )}
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
