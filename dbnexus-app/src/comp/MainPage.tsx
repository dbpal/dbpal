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
import { SiWelcometothejungle } from 'react-icons/si';
import { TabContent, TabsWithContent } from './Tabs';
import { WelCome } from './WelCome';
import { modalStyle } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';

const keyShortChangetab = {
  callback: (e: KeyboardEvent) => {
    if (platform() == 'macos') {
      if (e.metaKey && e.key == '[') {
        keyShortChangetab.prev();
      }
      if (e.metaKey && e.key == ']') {
        keyShortChangetab.next();
      }
      if (e.metaKey && e.key == ',') {
        keyShortChangetab.showSetting();
      }
    }
    if (['windows', 'linux'].includes(platform())) {
      if (e.ctrlKey && e.key == '[') {
        keyShortChangetab.prev();
      }
      if (e.ctrlKey && e.key == ']') {
        keyShortChangetab.next();
      }
    }
  },
} as {
  prev: () => void;
  next: () => void;
  callback: (e: KeyboardEvent) => void;
  showSetting: () => void;
};

import { ModifySettings, SettingComp } from './Settings.tsx';
import {
  appStore,
  AppStoreKey,
  ContextMenuContext,
  TabsSetContext,
} from './helper.ts';

import { Box, Modal } from '@mui/material';

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

  keyShortChangetab.prev = () => {
    setTab((e) => (e > 0 ? e - 1 : e));
  };

  keyShortChangetab.next = () => {
    if (tabs.length - 1 > tab) {
      setTab((e) => e + 1);
    }
  };

  keyShortChangetab.showSetting = () => {
    setShowSetting(true);
  };

  // shortcut
  useEffect(() => {
    document.addEventListener('keydown', keyShortChangetab.callback);
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
  }, [tabs]);

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

  const closeTab = useCallback(
    (index: number) => {
      let id = tabs[index].id;
      setTabs((prevTabs) => {
        return prevTabs.filter((x) => x.id !== id);
      });
    },
    [tabs]
  );

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
      let size = await (await appStore()).get<number>(AppStoreKey.LeftSize);
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
      minSize={0}
      defaultSize={leftSize}
      style={{
        minHeight: '100vh',
        height: 'auto',
      }}
      onDragFinished={async (newsize) => {
        (await appStore()).set(AppStoreKey.LeftSize, newsize);
      }}
    >
      <div style={{ overflowY: 'auto', height: '100vh' }}>
        <Left></Left>
      </div>
      <div style={{ overflowY: 'auto', height: '100vh' }}>
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
              {menu && menu.content}
            </div>

            {showSetting && (
              <Modal
                open={showSetting}
                onClose={(e, reason) => {
                  if (reason === 'backdropClick') {
                    return;
                  }
                  //@ts-ignore
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
