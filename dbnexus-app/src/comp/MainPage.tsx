'use strict';
import { NotificationProvider } from './Notification';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
import { platform } from '@tauri-apps/plugin-os';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useEffect,
  useState,
} from 'react';
import { TabContent, TabsWithContent } from './Tabs';
import { EscDownEvent, modalHandleClose, modalStyle } from './Common';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import { event } from '@tauri-apps/api';
import { SettingComp } from './Settings.tsx';
import {
  ContextMenuContext,
  generateRandomString,
  secondConfirm,
  TabsSetContext,
  WindowFocusContext,
  windowFocusChangeEvent,
  menuDivID,
} from './helper.ts';
import '../app.css';
import { Box, Modal, Paper } from '@mui/material';
import { EditConfigFile } from './EditConfigFile.tsx';
import { ModalHeader } from './CommonComp.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

const shortCuts = {
  callback: (e: KeyboardEvent) => {
    if (platform() == 'macos') {
      if (e.metaKey && e.key == '[') {
        shortCuts.prev();
      }
      if (e.metaKey && e.key == ']') {
        shortCuts.next();
      }
      if (e.metaKey && e.key == ',') {
        shortCuts.showSetting();
        e.preventDefault();
      }

      if (e.metaKey && e.key == 'w') {
        shortCuts.closeCurrentTab(e);
      }
    }
    if (['windows', 'linux'].includes(platform())) {
      if (e.ctrlKey && e.key == '[') {
        shortCuts.prev();
      }
      if (e.ctrlKey && e.key == ']') {
        shortCuts.next();
      }
      if (e.ctrlKey && e.key == 'w') {
        shortCuts.closeCurrentTab(e);
      }

      if (e.ctrlKey && e.key == ',') {
        shortCuts.showSetting();
        e.preventDefault();
      }
    }
  },
} as {
  prev: () => void;
  next: () => void;
  callback: (e: KeyboardEvent) => void;
  showSetting: () => void;
  closeCurrentTab: (_: KeyboardEvent) => void;
};

const settingWindowID = generateRandomString();

function App() {
  let [tabs, setTabs] = useState<TabContent[]>([]);
  const [tab, setTab] = useState(0);
  const closeTab = useCallback(
    (index: number) => {
      (async () => {
        let id = tabs[index].id;
        if (tabs[index].closeConfirmText) {
          const c = await secondConfirm({
            content: tabs[index].closeConfirmText,
            title: 'close the tab',
            confirmText: 'close',
          });
          if (!c) {
            return;
          }
        }
        setTabs((prevTabs) => {
          return prevTabs.filter((x) => x.id !== id);
        });
      })();
    },
    [tabs]
  );
  const showSetting = useCallback(() => {
    (async () => {
      const webview = await WebviewWindow.getByLabel(settingWindowID);
      if (webview) {
        webview.setFocus().catch((e) => {
          console.log('show setting failed', e);
        });
      } else {
        const webview = new WebviewWindow(settingWindowID, {
          title: `setting`,
          url: 'setting.html',
          width: 800,
          height: 600,
          center: true,
        });
        webview.once('tauri://created', () =>
          console.log('create setting view successfully')
        );
        webview.once('tauri://error', (e) => {
          console.log('create webview failed' + e.payload.toString());
        });
      }
    })();

    // // }
  }, []);

  shortCuts.prev = () => {
    setTab((e) => (e > 0 ? e - 1 : e));
  };

  shortCuts.next = () => {
    if (tabs.length - 1 > tab) {
      setTab((e) => e + 1);
    }
  };

  shortCuts.showSetting = () => {
    showSetting();
  };
  shortCuts.closeCurrentTab = (e) => {
    if (tabs.length > 0) {
      closeTab(tab);
      e.preventDefault();
    }
  };

  // shortcut
  useEffect(() => {
    document.addEventListener('keydown', shortCuts.callback);
  }, []);

  const [showEditConfig, setShowEditConfig] = useState(false);
  useEffect(() => {
    let unlisten = listen('edit_config_file', () => {
      setShowEditConfig(true);
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  const [menu, setUpMenu] = useState<{
    content: ReactNode;
    style: CSSProperties;
  }>();
  useEffect(() => {
    window.addEventListener('click', hiddenMenu);
  }, []);

  useEffect(() => {
    if (tab > 0 && tab >= tabs.length) {
      setTab((e) => e - 1);
    }
  }, [tabs]);

  const changeTab = useCallback((index) => {
    setTab(index as number);
  }, []);

  const [windowFocus, setWindowsFocus] = useState(true);
  useEffect(() => {
    let unlisten = listen<boolean>(
      windowFocusChangeEvent,
      (msg: event.Event<boolean>) => {
        setWindowsFocus(msg.payload);
      }
    );
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  return (
    <SettingComp>
      <ContextMenuContext.Provider value={{ setUpMenu }}>
        <TabsSetContext.Provider
          value={{ setTabs, tabs, setSelectTab: setTab }}
        >
          <NotificationProvider>
            <WindowFocusContext value={windowFocus}>
              <div>
                <PanelGroup
                  direction="horizontal"
                  style={{
                    height: '100vh',
                    width: '100vw',
                    padding: '0.5rem 1rem',
                  }}
                  autoSaveId="main"
                >
                  <Panel
                    style={{
                      overflowY: 'auto',
                      overflowX: 'hidden',
                      height: '100%',
                    }}
                    minSize={10}
                    maxSize={45}
                    collapsible
                    collapsedSize={0}
                  >
                    <Left></Left>
                  </Panel>
                  <PanelResizeHandle
                    style={{ width: 4 }}
                    className="resize-handle"
                  />
                  <Panel
                    style={{
                      overflow: 'auto',
                      height: '100%',
                    }}
                  >
                    <TabsWithContent
                      handleClose={closeTab}
                      tabs={tabs}
                      selectIndex={tab}
                      changeTab={changeTab}
                      setTabs={setTabs}
                    ></TabsWithContent>
                  </Panel>
                </PanelGroup>
              </div>
              <div
                id="menu-div"
                style={{
                  ...menuBasicStyle,
                  ...(menu && menu.style),
                }}
              >
                <Paper elevation={24}>{menu && menu.content}</Paper>
              </div>

              {showEditConfig && (
                <Modal
                  open={showEditConfig}
                  onClose={modalHandleClose((e: EscDownEvent) => {
                    e.preventDefault();
                    setShowEditConfig(false);
                  })}
                >
                  <Box
                    sx={{ resize: 'both', ...modalStyle }}
                    width="75%"
                    height="70vh"
                  >
                    <ModalHeader title="Edit Config File"></ModalHeader>
                    <EditConfigFile
                      handleClose={() => {
                        setShowEditConfig(false);
                      }}
                    />
                  </Box>
                </Modal>
              )}
            </WindowFocusContext>
          </NotificationProvider>
        </TabsSetContext.Provider>
      </ContextMenuContext.Provider>
    </SettingComp>
  );
}

const menuBasicStyle = {
  position: 'fixed',
  zIndex: 10000,
  height: 'fit-content',
  maxHeight: '48vh',
  overflow: 'auto',
} as CSSProperties;

function hiddenMenu() {
  const customMenu = document.getElementById(menuDivID);
  if (customMenu.style) {
    customMenu.style.display = 'none';
  }
}

function SeparateWindow(props: { children }) {
  useEffect(() => {
    window.addEventListener('click', hiddenMenu);
  }, []);
  const { children } = props;
  const [menu, setUpMenu] = useState<{
    content: ReactNode;
    style: CSSProperties;
  }>();
  return (
    <TabsSetContext.Provider
      value={{
        setTabs: () => {
          console.log('should not be called this is a bug.');
        },
        tabs: [],
        setSelectTab: 0,
      }}
    >
      <SettingComp>
        <ContextMenuContext.Provider value={{ setUpMenu }}>
          <NotificationProvider>
            <div
              style={{
                padding: '0.5rem 1rem',
                width: '100vw',
                height: '100vh',
              }}
            >
              {children}
            </div>
            <div
              id={menuDivID}
              style={{
                ...menuBasicStyle,
                ...(menu && menu.style),
              }}
            >
              <Paper elevation={24}>{menu && menu.content}</Paper>
            </div>
          </NotificationProvider>
        </ContextMenuContext.Provider>
      </SettingComp>
    </TabsSetContext.Provider>
  );
}

export { App, SeparateWindow };
