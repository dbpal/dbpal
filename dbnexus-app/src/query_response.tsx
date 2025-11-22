import ReactDOM from "react-dom/client";

import { DetachQueyRespViewParameter, SomeTempStore } from "./comp/helper.ts";
import { SeparateWindow } from "./comp/MainPage.tsx";
import React, { useEffect, useState } from "react";
import { MultiQueryResp } from "./comp/QueryResp.tsx";
import { emit, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Typography } from "@mui/material";
import { QuerySeparateWindowMsg } from "./comp/SqlQuery.tsx";

const { uuid, queryId, eventName } = SomeTempStore.get<DetachQueyRespViewParameter>();

function QueryResponse() {
  const [resp, setResp] = useState([]);
  const handleReload = () => {
    invoke<any>("get_query_cache", {
      name: uuid,
      queryId,
    }).then(data => {
      setResp(data);
    });
  };
  useEffect(() => {
    const unlisten = listen<"listen_ok" | object | string>(eventName, e => {
      if (e.payload == QuerySeparateWindowMsg.listenOK) {
        // I send this message nothing to do here.
        return;
      }
      if (e.payload === QuerySeparateWindowMsg.reloadData) {
        handleReload();
        return;
      }
      console.error("unhandled event:", e.payload);
    });
    // notify the parent I setup ok.
    emit(eventName, QuerySeparateWindowMsg.listenOK);
    return () => {
      unlisten.then(e => {
        e();
      });
    };
  }, []);

  return (
    <div style={{ height: "100%" }}>
      {resp.length == 0 && <Typography color="info"> No Data </Typography>}
      <MultiQueryResp data={resp} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <QueryResponse />
    </SeparateWindow>
  </React.StrictMode>,
);
