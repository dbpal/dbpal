import "@fontsource/pt-mono/400.css";

import ReactDOM from "react-dom/client";

import { AllKindChat } from "./comp/Chat.tsx";
import { SeprateWindow } from "./comp/MainPage.tsx";
import { providerQueryID } from "./comp/Common.ts";
import { DataSourceKind, DbnameContext } from "./comp/helper.ts";

const queryString = window.location.search;

// 2. Instantiate URLSearchParams.
const urlParams = new URLSearchParams(queryString);
const dbName = urlParams.get("dbName");
const kind = urlParams.get("kind");
const queryId = urlParams.get("queryId");

providerQueryID.id = queryId || "";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SeprateWindow>
    <DbnameContext value={{ dbName, kind: kind as DataSourceKind }}>
      <AllKindChat />
    </DbnameContext>
  </SeprateWindow>,
);
