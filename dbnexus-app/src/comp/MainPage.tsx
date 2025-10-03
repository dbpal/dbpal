'use strict';
import { NotificationProvider } from './Notification';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { platform } from '@tauri-apps/plugin-os';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useEffect,
  useState,
} from 'react';
import { MdError } from 'react-icons/md';
import { TabContent, TabsWithContent } from './Tabs';
import { EscDownEvent, modalHandleClose, modalStyle } from './Common';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import { event } from '@tauri-apps/api';
import { SettingComp } from './Settings.tsx';
import {
  ContextMenuContext,
  generateRandomString,
  SaveDomSizes,
  secondConfirm,
  TabsSetContext,
  WindowFocusContext,
  windowFocusChangeEvent,
  menuDivID,
} from './helper.ts';

import { Box, Modal, Paper } from '@mui/material';
import { BackendCrash } from './BackendCrash.tsx';
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
const openLogFileId = generateRandomString();

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
          console.log('create table view successfully')
        );
        webview.once('tauri://error', (e) => {
          alert('create webview failed' + e.payload.toString());
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

  useEffect(() => {
    let unlisten = listen<string>(
      'backend-crash',
      (msg: event.Event<string>) => {
        setTabs((tabs) => {
          let length = tabs.length;
          setTab(length);
          return [
            ...tabs,
            new TabContent(
              generateRandomString(15),
              'backend-crash',
              <BackendCrash msg={msg.payload}></BackendCrash>,
              <MdError> </MdError>
            ),
          ];
        });
      }
    );
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  useEffect(() => {
    let unlisten = listen<string>('open_log_file', () => {
      (async () => {
        let webview = await WebviewWindow.getByLabel(openLogFileId);
        if (webview) {
          webview.setFocus();
        } else {
          const webview = new WebviewWindow(openLogFileId, {
            title: 'log file',
            url: 'log_file.html',
            width: 800,
            height: 600,
            maximized: true,
          });
          webview.once('tauri://created', () =>
            console.log('create table view successfully')
          );
          webview.once('tauri://error', (e) => {
            console.log('error', 'create webview failed', e.event.toString());
          });
        }
      })();
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  useEffect(() => {
    let unlisten = listen('show_settings', () => {
      showSetting();
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
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
  const leftSizeKey = 'left-size';
  const [leftSize, setLeftSize] = useState(300);
  useEffect(() => {
    async () => {
      SaveDomSizes.getSize(leftSizeKey).then((size) => {
        if (size && size.width) {
          setLeftSize(size.width);
        }
      });
    };
  }, []);

  let mainContent = (
    //@ts-ignore
    <SplitPane
      split="vertical"
      minSize={200}
      defaultSize={leftSize}
      style={{
        height: '100vh',
        width: '100vw',
        overflow: 'auto',
        padding: '0px 0.9rem',
      }}
      maxSize={800}
      onChange={(newSize) => {
        SaveDomSizes.saveSize(leftSizeKey, { width: newSize });
      }}
    >
      <div
        style={{
          overflow: 'auto',
          height: '100%',
        }}
      >
        <Left></Left>
      </div>
      <div style={{ overflow: 'auto', height: '100%' }}>
        <TabsWithContent
          handleClose={closeTab}
          tabs={tabs}
          selectIndex={tab}
          changeTab={changeTab}
          setTabs={setTabs}
        ></TabsWithContent>
      </div>
    </SplitPane>
  );
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
              <div>{mainContent}</div>
              <div
                id={menuDivID}
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
          alert('should not be called this is a bug');
        },
        tabs: [],
        setSelectTab: 0,
      }}
    >
      <SettingComp>
        <ContextMenuContext.Provider value={{ setUpMenu }}>
          <NotificationProvider>
            <div style={{ padding: '0.5rem 0.9rem' }}> {children} </div>
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
