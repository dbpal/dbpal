import { NotificationProvider } from './Notification';

import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Box, Modal } from '@mui/material';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useEffect,
  useState,
} from 'react';

import { TabContent, TabsWithContent } from './Tabs';
import { WelCome } from './WelCome';
import { modalStyle, tabsHasId } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import AddConfig from './AddConnection.tsx';
import { ModifySettings, SettingComp } from './Settings.tsx';
import {
  appStore,
  AppStoreKey,
  ContextMenuContext,
  TabsSetContext,
} from './helper.ts';

export default function MainPage() {
  const [showAddConnection, setShowAddConnection] = useState(false);

  let [tabs, setTabs] = useState([
    new TabContent('welcome', 'welcome', <WelCome></WelCome>),
  ]);
  const [selectIndex, setSelectTab] = useState(0);

  useEffect(() => {
    let unlisten = listen('show_settings', () => {
      let id = 'setting';
      let index = tabsHasId(tabs, id);
      if (typeof index == 'number') {
        setSelectTab(index);
      } else {
        setTabs((e) => {
          setSelectTab(e.length);
          return [
            ...e,
            new TabContent(id, id, <ModifySettings></ModifySettings>),
          ];
        });
      }
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, [tabs]);

  useEffect(() => {
    let unlisten = listen('add_config', () => {
      setShowAddConnection(true);
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

  const closeTab = useCallback(
    (index: number) => {
      let id = tabs[index].id;
      setTabs((prevTabs) => prevTabs.filter((x) => x.id !== id));
      if (selectIndex != index) {
        // not close current
        if (index < selectIndex) {
          setSelectTab(selectIndex - 1);
        }
      } else {
        if (selectIndex > 0) {
          setSelectTab(selectIndex - 1);
        }
      }
    },
    [tabs]
  );
  const changeTab = useCallback((index) => {
    setSelectTab(index as number);
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
      <Left></Left>
      <TabsWithContent
        handleClose={closeTab}
        tabs={tabs}
        selectIndex={selectIndex}
        changeTab={changeTab}
        setTabs={setTabs}
      ></TabsWithContent>
    </SplitPane>
  );

  return (
    <SettingComp>
      <ContextMenuContext.Provider value={{ setUpMenu }}>
        <TabsSetContext.Provider value={{ setTabs, tabs, setSelectTab }}>
          <NotificationProvider>
            <div> {mainContent} </div>
            <div id="menu-div" style={menu && menu.style}>
              {menu && menu.content}
            </div>
            {showAddConnection && (
              <Modal
                open={showAddConnection}
                onClose={(_, reason) => {
                  if (reason === 'backdropClick') {
                    return;
                  }
                  setShowAddConnection(false);
                }}
              >
                <Box sx={modalStyle} width="950px">
                  <AddConfig
                    handleClose={() => {
                      setShowAddConnection(false);
                    }}
                  ></AddConfig>
                </Box>
              </Modal>
            )}
          </NotificationProvider>
        </TabsSetContext.Provider>
      </ContextMenuContext.Provider>
    </SettingComp>
  );
}
