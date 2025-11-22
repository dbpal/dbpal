import { NotificationProvider } from "./Notification";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { CSSProperties, ReactNode, useCallback, useEffect, useState } from "react";
import { TabContent, TabsWithContent } from "./Tabs";
import { Left } from "./Left";
import { SettingComp } from "./Settings.tsx";
import {
  ContextMenuContext,
  secondConfirm,
  TabsSetContext,
  WindowFocusContext,
  getViewportSize,
  KeyPressed,
} from "./helper.ts";
import "../app.css";
import { Paper, useTheme } from "@mui/material";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

function App() {
  const [tabs, setTabs] = useState<TabContent[]>([]);
  const [tab, setTab] = useState(0);
  const closeTab = useCallback(
    (index: number) => {
      (async () => {
        const id = tabs[index].id;
        if (tabs[index].closeConfirmText) {
          const confirm = await secondConfirm({
            content: tabs[index].closeConfirmText,
            title: "close the tab",
            confirmText: "close",
          });
          if (!confirm) {
            return;
          }
        }
        setTabs(prevTabs => {
          return prevTabs.filter(x => x.id !== id);
        });
      })();
    },
    [tabs],
  );

  const shortCuts = useCallback(
    (e: KeyboardEvent) => {
      const prevTab = () => {
        setTab(e => (e > 0 ? e - 1 : e));
      };
      const nextTab = () => {
        if (tabs.length - 1 > tab) {
          setTab(e => e + 1);
        }
      };
      const closeCurrentTab = () => {
        if (tabs.length > 0) {
          closeTab(tab);
          e.preventDefault();
        }
      };
      if (KeyPressed.prevTab(e)) {
        prevTab();
      }
      if (KeyPressed.nextTab(e)) {
        nextTab();
      }
      if (KeyPressed.closeTab(e)) {
        closeCurrentTab();
      }
    },
    [tabs.length, tab, closeTab],
  );

  // shortcut
  useEffect(() => {
    document.addEventListener("keydown", shortCuts);
    return () => {
      document.removeEventListener("keydown", shortCuts);
    };
  }, [shortCuts]);

  useEffect(() => {
    if (tab > 0 && tab >= tabs.length) {
      setTab(e => e - 1);
    }
  }, [tabs, tab]);

  const changeTab = useCallback(index => {
    setTab(index as number);
  }, []);

  return (
    <SettingComp>
      <TabsSetContext.Provider value={{ setTabs, tabs, setSelectTab: setTab }}>
        <NotificationProvider>
          <Menu>
            <WindowFocus>
              <div>
                <PanelGroup
                  direction="horizontal"
                  style={{
                    height: "100vh",
                    width: "100vw",
                    padding: "0.5rem 1rem",
                  }}
                  autoSaveId="main"
                >
                  <Panel
                    style={{
                      overflowY: "auto",
                      overflowX: "hidden",
                      height: "100%",
                    }}
                    minSize={10}
                    maxSize={85}
                    collapsible
                    collapsedSize={0}
                  >
                    <Left />
                  </Panel>
                  <PanelResizeHandle style={{ width: 4 }} className="resize-handle" />
                  <Panel
                    style={{
                      overflow: "auto",
                      height: "100%",
                    }}
                  >
                    <TabsWithContent
                      handleClose={closeTab}
                      tabs={tabs}
                      selectIndex={tab}
                      changeTab={changeTab}
                      setTabs={setTabs}
                    />
                  </Panel>
                </PanelGroup>
              </div>
            </WindowFocus>
          </Menu>
        </NotificationProvider>
      </TabsSetContext.Provider>
    </SettingComp>
  );
}

function WindowFocus(props: { children: ReactNode }) {
  const { children } = props;
  const [windowFocus, setWindowsFocus] = useState(true);
  useEffect(() => {
    const webview = WebviewWindow.getCurrent();
    const unlisten = webview.onFocusChanged(e => {
      setWindowsFocus(e.payload);
    });
    return () => {
      unlisten.then(callback => {
        callback();
      });
    };
  }, []);

  return <WindowFocusContext value={windowFocus}>{children}</WindowFocusContext>;
}

function Menu(props: { children: ReactNode }) {
  const { children } = props;
  const [menu, setUpMenu] = useState<{
    content: ReactNode;
    event: MouseEvent;
  }>(null);
  const [display, setDisplay] = useState("none");
  const [menuDiv, setMenuDiv] = useState<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!menuDiv) {
      return;
    }
    document.addEventListener("click", () => {
      if (!menuDiv.style) {
        return;
      }
      setDisplay("none");
    });
  }, [menuDiv]);

  useEffect(() => {
    if (!menu) {
      return;
    }
    if (!menuDiv) {
      return;
    }
    setDisplay("block");
    if (menu && menu.event) {
      menu.event.preventDefault();
    }
  }, [menu, menuDiv]);

  const theme = useTheme();

  return (
    <ContextMenuContext.Provider value={{ setUpMenu }}>
      {children}
      <div
        ref={setMenuDiv}
        style={{
          position: "fixed",
          zIndex: 10000,
          height: "fit-content",
          maxHeight: "48vh",
          overflow: "auto",
          display: display,
          border: "2px solid " + theme.palette.success.main,
          borderRadius: "0.5rem",
          ...(menu && calMenuStyle(menu.event)),
        }}
      >
        <Paper elevation={24}>{menu && menu.content}</Paper>
      </div>
    </ContextMenuContext.Provider>
  );
}

function SeparateWindow(props: { children }) {
  const { children } = props;
  return (
    <SettingComp>
      <NotificationProvider>
        <Menu>
          <div
            style={{
              padding: "0.5rem 1rem",
              width: "100vw",
              height: "100vh",
            }}
          >
            <div style={{ position: "relative", height: "100%", width: "100%" }}>
              {" "}
              {children}
            </div>
          </div>
        </Menu>
      </NotificationProvider>
    </SettingComp>
  );
}

function calMenuStyle(e: MouseEvent): CSSProperties {
  const x = e.clientX;
  const y = e.clientY;
  const ret: CSSProperties = {};
  const { height, width } = getViewportSize();
  ret.top = "auto";
  ret.bottom = "auto";
  ret.left = "auto";
  ret.right = "auto";
  if (width - x > width / 2) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      "px";
  } else {
    ret.right = width - x - 10 + "px";
  }

  if (height - y > height / 2) {
    // 这个是点击的位置下面有很大的空间
    ret.top = y - 10 + "px";
  } else {
    ret.bottom = height - y - 10 + "px";
  }
  return ret;
}

export { App, SeparateWindow };
