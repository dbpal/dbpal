import ReactDOM from "react-dom/client";
import { SomeTempStore } from "./comp/helper.ts";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { AddConnection } from "./comp/AddConnection.tsx";

const currentWebview = WebviewWindow.getCurrent();

function View() {
  return (
    <div style={{ maxWidth: "800px", margin: "0 auto" }}>
      <AddConnection
        handleClose={() => {
          currentWebview.close();
        }}
        config={SomeTempStore.get()}
      />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SeparateWindow>
    <View />
  </SeparateWindow>,
);
