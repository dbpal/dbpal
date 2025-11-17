"use strict";
import ReactDOM from "react-dom/client";
import React, { useContext, useEffect, useState } from "react";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { BackendCrash } from "./comp/BackendCrash.tsx";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./comp/Notification.tsx";

const params = new URLSearchParams(location.search);
const id = parseInt(params.get("id"));

function View() {
  const [msg, setMsg] = useState("");
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string>("get_backend_crash", { id })
      .then(x => setMsg(x))
      .catch(e => {
        showNotification("error", "get panic info failed", e);
      });
  }, [showNotification]);
  return <BackendCrash msg={msg} />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <View />
    </SeparateWindow>
  </React.StrictMode>,
);
