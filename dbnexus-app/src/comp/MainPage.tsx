// import { Notifications } from "./Notification";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useCallback, useEffect, useState } from "react";
import { TabsWithContent } from "./Tabs";
import { Left } from "./Left";
import { SettingProvider } from "./Settings.tsx";
import {
  secondConfirm,
  KeyPressed,
  resizeHandlerSize,
  handleCopyToClipboard,
  TabContext,
} from "./helper.ts";
import "../app.css";
import {
  AlertTitle,
  Button,
  Dialog,
  DialogTitle,
  IconButton,
  Menu,
  Stack,
  useTheme,
} from "@mui/material";
import { EscDownEvent, modalHandleClose } from "./Common.ts";
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
  useOneShotDialogUI,
  useTabsManage,
} from "./store.tsx";
import { SnackbarProvider } from "notistack";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

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
        <Dialog
          fullWidth
          maxWidth="md"
          open={!!panicInfo}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setPanicInfo("");
          })}>
          <DialogTitle>panic information</DialogTitle>

          <Stack
            direction="row"
            style={{
              position: "sticky",
              top: 0,
              backgroundColor: theme.palette.background.paper,
            }}>
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
                  showNotification({
                    severity: "success",
                    title: "remove panic file success.",
                  });
                } catch (e) {
                  showNotification({
                    severity: "error",
                    title: "remove panic file failed",
                    message: e,
                  });
                }
              }}>
              <DeleteForeverIcon />
            </IconButton>
          </Stack>
          <pre style={{ color: theme.palette.error.main }}>{panicInfo}</pre>
        </Dialog>
      )}
    </div>
  );
}

function ShowShortCut() {
  const [show, setShow] = useState(false);
  useEffect(() => {
    const unlisten = listen("show_shortcut", async () => {
      if (await WebviewWindow.getCurrent().isFocused()) {
        setShow(true);
      }
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
        <Dialog
          fullWidth
          maxWidth="md"
          open={show}
          onClose={() => {
            setShow(false);
          }}>
          <AlertTitle> shorts cuts </AlertTitle>
          <ShowCuts />
        </Dialog>
      )}
    </>
  );
}

function App() {
  const { tabs, setCurrentTab, setTabs, currentTab } = useTabsManage();
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

  return (
    <AppSnackbarProvider>
      <SettingProvider>
        <AppMenu />
        <DialogExtraUI />
        <ShowPanic />
        <ShowShortCut />
        <PanelGroup
          direction="horizontal"
          style={{
            height: "100vh",
            width: "100vw",
            padding: "4px",
          }}
          autoSaveId="main">
          <Panel
            style={{
              overflow: "auto",
              height: "100%",
              padding: "4px",
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
            <TabsWithContent />
          </Panel>
        </PanelGroup>
      </SettingProvider>
    </AppSnackbarProvider>
  );
}

function DialogExtraUI() {
  const diagUi = useOneShotDialogUI();
  return (
    diagUi.ui && (
      <Dialog
        open={true}
        maxWidth={diagUi.maxWidth}
        fullWidth={diagUi.fullWidth}
        onClose={() => {
          diagUi.setOneShotUI({ ui: null });
        }}>
        {diagUi.title && <DialogTitle>{diagUi.title}</DialogTitle>}
        {diagUi.ui}
      </Dialog>
    )
  );
}

function AppSnackbarProvider({ children }) {
  return (
    <SnackbarProvider
      maxSnack={9}
      anchorOrigin={{
        vertical: "top",
        horizontal: "right",
      }}
      style={{ maxWidth: "55vw" }}
      dense>
      {children}
    </SnackbarProvider>
  );
}

function SeprateWindow({ children }) {
  const tabId = "the one tab";
  const { getConfirmMsg } = useConfirmMsg();

  useEffect(() => {
    const windows = WebviewWindow.getCurrent();
    windows.onCloseRequested(async (event) => {
      const msg = getConfirmMsg(tabId);
      if (msg) {
        const confirm = await secondConfirm({
          content: msg,
          title: "close the tab",
          confirmText: "close",
        });
        if (!confirm) {
          event.preventDefault();
          return;
        }
      }
    });
  }, [getConfirmMsg]);

  return (
    <TabContext value={{ id: tabId }}>
      <AppSnackbarProvider>
        <SettingProvider>
          <ShowShortCut />
          <AppMenu />
          <DialogExtraUI />
          <div style={{ width: "100vw", height: "100vh", padding: "4px" }}>
            {children}
          </div>
        </SettingProvider>
      </AppSnackbarProvider>
    </TabContext>
  );
}

function AppMenu() {
  // 假设你的 store 返回的对象里包含 event 和 content
  const data = useContextMenu((state) => state.state);
  const setUpMenu = useContextMenu((state) => state.setUpMenu); // 假设你有关闭方法

  const closeMenu = useCallback(() => {
    setUpMenu(null);
  }, [setUpMenu]);
  // 如果没有菜单数据，直接不渲染
  if (!data || !data.event) return null;
  data.event.preventDefault();

  return (
    <Menu
      // 1. 基础开关逻辑
      open={Boolean(data)}
      onClose={closeMenu} // 点击外部或按 Esc 会自动触发
      // 2. 核心定位配置：让 MUI 接管原本 calMenuStyle 的工作
      anchorReference="anchorPosition"
      anchorPosition={{
        top: data.event.clientY,
        left: data.event.clientX,
      }}
      onClick={() => {
        closeMenu();
      }}
      // 4. 优化体验
      slotProps={{
        backdrop: {
          // 如果你不想要背景遮罩感，可以把背景设为透明
          style: { backgroundColor: "transparent" },
        },
      }}>
      {/* menu.content 里面应该是具体的 <MenuItem> 组件 */}
      {data.content}
    </Menu>
  );
}

export { App, SeprateWindow };
