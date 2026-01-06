import ReactDOM from "react-dom/client";

import { SomeTempStore } from "./comp/helper.ts";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { ConnConfigInfo } from "./comp/CommonComp2.tsx";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SeparateWindow>
    <div style={{ height: "100%" }}>
      <ConnConfigInfo uuid={SomeTempStore.get<string>()} />
    </div>
  </SeparateWindow>,
);
