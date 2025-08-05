import { NotificationContext, NotificationProvider } from './Notification';

import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { platform } from '@tauri-apps/plugin-os';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useState,
} from 'react';
import { SiWelcometothejungle } from 'react-icons/si';
import { TabContent, TabsWithContent } from './Tabs';
import { WelCome } from './WelCome';
import { EscDownEvent, modalStyle } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';

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
      if (e.metaKey && e.key == 'w') {
        shortCuts.closeCurrentTab();
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
        shortCuts.closeCurrentTab();
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
  closeCurrentTab: () => void;
};

import { ModifySettings, SettingComp } from './Settings.tsx';
import {
  appStore,
  AppStoreKey,
  ContextMenuContext,
  TabsSetContext,
} from './helper.ts';

import { Box, Modal, Paper } from '@mui/material';

export default function MainPage() {
  let [tabs, setTabs] = useState([
    new TabContent(
      'welcome',
      'welcome',
      <WelCome></WelCome>,
      <SiWelcometothejungle> </SiWelcometothejungle>
    ),
  ]);
  const [tab, setTab] = useState(0);
  const [showSetting, setShowSetting] = useState(false);
  const closeTab = useCallback(
    (index: number) => {
      let id = tabs[index].id;
      setTabs((prevTabs) => {
        return prevTabs.filter((x) => x.id !== id);
      });
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
  shortCuts.closeCurrentTab = () => {
    closeTab(tab);
  };

  // shortcut
  useEffect(() => {
    document.addEventListener('keydown', shortCuts.callback);
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

  const [leftSize, setLeftSize] = useState(300);
  useEffect(() => {
    let a = async () => {
      let size = await appStore.get<number>(AppStoreKey.LeftSize);
      if (!size) {
        size = 300;
      }
      setLeftSize(size);
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
        appStore.set(AppStoreKey.LeftSize, newsize);
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

            <WarnUnCommitTx></WarnUnCommitTx>

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

function WarnUnCommitTx() {
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    let unlisten = listen('close_connection_has_uncommit_tx', () => {
      showNotification(
        'error',
        'You have uncommitted transaction  may lead to database beed locked'
      );
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  return <></>;
}
