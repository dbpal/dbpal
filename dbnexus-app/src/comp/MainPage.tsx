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
import { EscDownEvent, modalStyle } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import { event } from '@tauri-apps/api';
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
      }
      // 这个和macos的 quit windows冲突了 要解决
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

import { ModifySettings, SettingComp } from './Settings.tsx';
import {
  ContextMenuContext,
  generateRandomString,
  SaveSizesToStore,
  secondConfirm,
  TabsSetContext,
} from './helper.ts';

import { Box, Modal, Paper } from '@mui/material';
import { BackendCrash } from './BackendCrash.tsx';
import { EditConfigFile } from './EditConfigFile.tsx';
import { ModalHeader } from './CommonComp.tsx';

export default function MainPage() {
  let [tabs, setTabs] = useState<TabContent[]>([]);
  const [tab, setTab] = useState(0);
  const [showSetting, setShowSetting] = useState(false);
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

  shortCuts.prev = () => {
    setTab((e) => (e > 0 ? e - 1 : e));
  };

  shortCuts.next = () => {
    if (tabs.length - 1 > tab) {
      setTab((e) => e + 1);
    }
  };

  shortCuts.showSetting = () => {
    setShowSetting(true);
  };
  shortCuts.closeCurrentTab = (e) => {
    closeTab(tab);
    e.preventDefault();
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
    let unlisten = listen('show_settings', () => {
      setShowSetting(true);
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
    // cancel the menu
    const customMenu = document.getElementById('menu-div');
    window.addEventListener('click', () => {
      customMenu.style.display = 'none';
    });
  });

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
    let a = async () => {
      SaveSizesToStore.getSize(leftSizeKey).then((size) => {
        if (size && size.width) {
          setLeftSize(size.width);
        }
      });
    };
    a();
  }, []);

  let mainContent = (
    //@ts-ignore
    <SplitPane
      split="vertical"
      minSize={200}
      defaultSize={leftSize}
      style={{
        minHeight: '100vh',
        height: 'auto',
      }}
      maxSize={800}
      onDragFinished={async (newsize) => {
        SaveSizesToStore.saveSize(leftSizeKey, { width: newsize });
      }}
    >
      <div
        style={{
          overflow: 'auto',
          height: '100vh',
        }}
      >
        <Left></Left>
      </div>
      <div style={{ overflow: 'auto', height: '100vh' }}>
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

  return (
    <SettingComp>
      <ContextMenuContext.Provider value={{ setUpMenu }}>
        <TabsSetContext.Provider
          value={{ setTabs, tabs, setSelectTab: setTab }}
        >
          <NotificationProvider>
            <div> {mainContent} </div>

            <div id="menu-div" style={menu && menu.style}>
              <Paper elevation={24}> {menu && menu.content}</Paper>
            </div>

            {showEditConfig && (
              <Modal
                open={showEditConfig}
                onClose={(e: EscDownEvent, reason) => {
                  if (reason === 'backdropClick') {
                    return;
                  }
                  e.preventDefault();
                  setShowEditConfig(false);
                }}
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
            {showSetting && (
              <Modal
                open={showSetting}
                onClose={(e: EscDownEvent, reason) => {
                  if (reason === 'backdropClick') {
                    return;
                  }
                  e.preventDefault();
                  setShowSetting(false);
                }}
              >
                <Box sx={modalStyle} width="55%">
                  <ModalHeader title="Modify Settings"></ModalHeader>
                  <ModifySettings
                    handleClose={() => {
                      setShowSetting(false);
                    }}
                  ></ModifySettings>
                </Box>
              </Modal>
            )}
          </NotificationProvider>
        </TabsSetContext.Provider>
      </ContextMenuContext.Provider>
    </SettingComp>
  );
}
