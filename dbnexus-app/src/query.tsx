import "@fontsource/pt-mono/400.css";

import ReactDOM from "react-dom/client";

import { AllKindChat } from "./comp/Chat.tsx";
import { SeprateWindow } from "./comp/MainPage.tsx";
import { providerQueryID } from "./comp/Common.ts";
import { DataSourceKind } from "./comp/helper.ts";

const queryString = window.location.search;

// 2. 实例化 URLSearchParams
const urlParams = new URLSearchParams(queryString);
const uuid = urlParams.get("uuid");
const kind = urlParams.get("kind");
const queryId = urlParams.get("queryId");

providerQueryID.id = queryId || "";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SeprateWindow>
    <AllKindChat uuid={uuid} kind={kind as DataSourceKind} />
  </SeprateWindow>,
);
