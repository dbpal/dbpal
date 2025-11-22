import ReactDOM from "react-dom/client";
import React, { useEffect, useState } from "react";
import {
  DataSourceKind,
  FavoriteSql,
  secondConfirm,
  SetCloseConfirmTextContext,
  SomeTempStore,
} from "./comp/helper.ts";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { AllKindChat } from "./comp/Chat.tsx";
import { Stack } from "@mui/material";

const sqlData = SomeTempStore.get<FavoriteSql>();

// close confirm message
// when in transaction prevent user to close the window
let exitConfirmMessage: string;

let exit: () => void = null;

function Query(props: {
  uuid: string;
  kind: DataSourceKind;
  sql?: string;
  favoriteName?: string;
  language?: string;
}) {
  const { uuid, kind, sql, favoriteName, language } = props;
  const [closeConfirm, setCloseConfirm] = useState("");
  useEffect(() => {
    exitConfirmMessage = closeConfirm;
  }, [closeConfirm]);
  const [exiting, setExisting] = useState(false);

  exit = () => {
    setExisting(true);
  };
  return (
    <SetCloseConfirmTextContext
      value={msg => {
        setCloseConfirm(msg);
      }}
    >
      {exiting ? (
        <Exiting />
      ) : (
        <AllKindChat
          uuid={uuid}
          kind={kind}
          value={sql}
          language={language}
          favoriteName={favoriteName}
        />
      )}
    </SetCloseConfirmTextContext>
  );
}

function Exiting() {
  const [count, setCount] = useState(10);
  useEffect(() => {
    WebviewWindow.getCurrent()
      .hide()
      .catch(e => {
        console.log("hide error", e);
      });
    setInterval(() => {
      setCount(count => count - 1);
    }, 1000);
  }, []);
  useEffect(() => {
    if (count < 0) {
      WebviewWindow.getCurrent().destroy();
    }
  }, [count]);
  return <Stack> exiting {count} </Stack>;
}

const currentWebview = await WebviewWindow.getCurrent();
currentWebview.onCloseRequested(async event => {
  event.preventDefault();
  if (!exitConfirmMessage) {
    exit();
    return;
  }
  const confirm = await secondConfirm({
    title: "close the window",
    content: exitConfirmMessage,
    confirmText: "close",
  });
  if (confirm) {
    exit();
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <Query
        uuid={sqlData.uuid}
        kind={sqlData.kind}
        sql={sqlData.sql}
        favoriteName={sqlData.name}
        language={sqlData.language}
      />
    </SeparateWindow>
  </React.StrictMode>,
);
