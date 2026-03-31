import { Notifications } from "./Notification";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { CSSProperties, useCallback, useEffect, useState } from "react";
import { TabsWithContent } from "./Tabs";
import { Left } from "./Left";
import { SettingProvider } from "./Settings.tsx";
import {
  secondConfirm,
  getViewportSize,
  KeyPressed,
  resizeHandlerSize,
  handleCopyToClipboard,
} from "./helper.ts";
import "../app.css";
import { Box, Button, IconButton, Modal, Paper, Stack, useTheme } from "@mui/material";
import { EscDownEvent, modalHandleClose, modalStyle } from "./Common.ts";
import { ModalHeader } from "./CommonComp.tsx";
import { appLogDir, join } from "@tauri-apps/api/path";
import { readTextFile, remove } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { listen } from "@tauri-apps/api/event";
import { ShowCuts } from "./ShortCuts.tsx";
import {
  useConfirmMsg,
  useContextMenu,
  useNotification,
  useTabsManage,
} from "./store.tsx";

function ShowPanic() {
  const [panicInfo, setPanicInfo] = useState("");
  const { showNotification } = useNotification();
  const theme = useTheme();
  const getPanicFilePath = async () => {
    return appLogDir().then((e) => {
      return join(e, "dbpal.panic");
    });
  };

  useEffect(() => {
    setTimeout(() => {
      (async () => {
        try {
          const filepath = await getPanicFilePath();
          const x = await readTextFile(filepath);
          setPanicInfo(x);
        } catch (e) {
          console.error("trying to get panic info failed", e);
        }
      })();
    }, Math.random() * 1000);
  }, []);
  return (
    <div>
      {panicInfo && (
        <Modal
          open={!!panicInfo}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setPanicInfo("");
          })}>
          <Box sx={modalStyle} width="75%" height="65vh">
            <ModalHeader
              title="panic info"
              handleClose={() => {
                setPanicInfo("");
              }}
            />
            <Paper
              elevation={8}
              style={{
                overflow: "auto",
                width: "100%",
                flex: 1,
                padding: "4px",
              }}>
              <pre style={{ color: theme.palette.error.main }}>{panicInfo}</pre>
            </Paper>

            <Stack direction="row">
              <Button
                onClick={() => {
                  openUrl("https://github.com/dbpal/docs/issues");
                }}
                color="info">
                report
              </Button>
              <Button
                variant="outlined"
                onClick={() => {
                  handleCopyToClipboard(panicInfo, showNotification);
                }}>
                copy error msg
              </Button>
              <IconButton
                color="warning"
                onClick={async () => {
                  try {
                    await remove(await getPanicFilePath());
                    setPanicInfo("");
                    showNotification("success", "remove panic file success.");
                  } catch (e) {
                    showNotification("error", "remove panic file failed", e);
                  }
                }}>
                <DeleteForeverIcon />
              </IconButton>
            </Stack>
          </Box>
        </Modal>
      )}
    </div>
  );
}

function ShowShortCut() {
  const [show, setShow] = useState(false);
  useEffect(() => {
    const unlisten = listen("show_shortcut", () => {
      setShow(true);
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, []);
  return (
    <>
      {show && (
        <Modal
          open={show}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShow(false);
          })}>
          <Box sx={modalStyle} width="75%" height="65vh">
            <ModalHeader
              title="keyboard short cuts"
              handleClose={() => {
                setShow(false);
              }}
            />
            <ShowCuts />
          </Box>
        </Modal>
      )}
    </>
  );
}

function App() {
  const { tabs, setCurrentTab, setTabs, currentTab } = useTabsManage((state) => state);

  const cancelMsg = useConfirmMsg((state) => state.confirmMsgs);
  const closeTab = useCallback(
    (index: number) => {
      (async () => {
        const id = tabs[index].id;
        const msg = cancelMsg.get(id);

        if (msg) {
          const confirm = await secondConfirm({
            content: msg,
            title: "close the tab",
            confirmText: "close",
          });
          if (!confirm) {
            return;
          }
        }
        setTabs(tabs.filter((x) => x.id !== id));
      })();
    },
    [cancelMsg, setTabs, tabs],
  );

  const shortCuts = useCallback(
    (e: KeyboardEvent) => {
      const prevTab = () => {
        setCurrentTab(currentTab > 0 ? currentTab - 1 : currentTab);
      };
      const nextTab = () => {
        setCurrentTab(currentTab + 1);
      };
      const closeCurrentTab = () => {
        if (tabs.length > 0) {
          closeTab(currentTab);
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
    [setCurrentTab, currentTab, tabs.length, closeTab],
  );
  // shortcut
  useEffect(() => {
    document.addEventListener("keydown", shortCuts);
    return () => {
      document.removeEventListener("keydown", shortCuts);
    };
  }, [shortCuts]);
  useEffect(() => {
    if (currentTab > 0 && currentTab >= tabs.length) {
      setCurrentTab(currentTab - 1);
    }
  }, [currentTab, setCurrentTab, tabs]);

  const changeTab = useCallback(
    (index) => {
      setCurrentTab(index as number);
    },
    [setCurrentTab],
  );

  return (
    <SettingProvider>
      <Notifications />
      <Menu />
      <div>
        <ShowPanic />
        <ShowShortCut />
        <PanelGroup
          direction="horizontal"
          style={{
            height: "100vh",
            width: "100vw",
            padding: "0.5rem 1rem",
          }}
          autoSaveId="main">
          <Panel
            style={{
              overflow: "auto",
              height: "100%",
              paddingRight: "2px",
            }}
            minSize={10}
            maxSize={85}
            collapsible
            collapsedSize={0}>
            <Left />
          </Panel>
          <PanelResizeHandle
            style={{ width: resizeHandlerSize }}
            className="resize-handle"
          />
          <Panel
            style={{
              overflow: "auto",
              height: "100%",
              paddingLeft: "2px",
            }}>
            <TabsWithContent
              handleClose={closeTab}
              tabs={tabs}
              selectIndex={currentTab}
              changeTab={changeTab}
              setTabs={setTabs}
            />
          </Panel>
        </PanelGroup>
      </div>
    </SettingProvider>
  );
}

function Menu() {
  const menu = useContextMenu((state) => state.state);
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
    menu && (
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
        }}>
        <Paper elevation={24}>{menu && menu.content}</Paper>
      </div>
    )
  );
}

function calMenuStyle(e: React.MouseEvent<any>): CSSProperties {
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

export { App };
