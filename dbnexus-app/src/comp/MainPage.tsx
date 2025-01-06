import { NotificationProvider } from './Notification';

import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Box, Modal } from '@mui/material';
import { CSSProperties, ReactNode, useEffect, useState } from 'react';

import { TabContent, TabsWithContent } from './Tabs';
import { WelCome } from './WelCome';
import {
  MenuContext,
  modalStyle,
  tabsHasId,
  TabsSetContext,
} from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import AddConfig from './AddConnection.tsx';
import { ModifySettings, SettingComp } from './Settings.tsx';

export default function MainPage() {
  const [showAddConnection, setShowAddConnection] = useState(false);

  let [tabs, setTabs] = useState([
    new TabContent('welcome', 'welcome', <WelCome></WelCome>),
  ]);
  const [selectIndex, setSelectIndex] = useState(0);

  useEffect(() => {
    let unlisten = listen('show_settings', () => {
      let id = 'setting';
      let index = tabsHasId(tabs, id);
      if (typeof index == 'number') {
        setSelectIndex(index);
      } else {
        setTabs((e) => {
          setSelectIndex(e.length);
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

  const [menu, setUpMenu] = useState<{
    content: ReactNode;
    style: CSSProperties;
  }>();
  useEffect(() => {
    // cancel the menu
    const customMenu = document.getElementById('left-side-menu');
    window.addEventListener('click', () => {
      customMenu.style.display = 'none';
    });
  });

  return (
    <SettingComp>
      <MenuContext.Provider value={{ setUpMenu }}>
        <TabsSetContext.Provider value={{ setTabs, tabs, setSelectIndex }}>
          <NotificationProvider>
            <div>
              <SplitPane
                split="vertical"
                minSize={0}
                defaultSize={300}
                style={{
                  minHeight: '100vh',
                  height: 'auto',
                  padding: '8px',
                }}
              >
                <Left></Left>
                <TabsWithContent
                  handleClose={(index: number) => {
                    let id = tabs[index].id;
                    setTabs((prevTabs) => prevTabs.filter((x) => x.id !== id));
                    if (selectIndex >= index && selectIndex > 0) {
                      setSelectIndex(selectIndex - 1);
                    }
                  }}
                  tabs={tabs}
                  selectIndex={selectIndex}
                  changeTab={(index) => {
                    setSelectIndex(index as number);
                  }}
                  setTabs={setTabs}
                ></TabsWithContent>
              </SplitPane>
            </div>

            <div id="left-side-menu" style={menu && menu.style}>
              {menu && menu.content}
            </div>

            {showAddConnection && (
              <Modal
                open={showAddConnection}
                onClose={() => {
                  setShowAddConnection(false);
                }}
                aria-labelledby="add column"
              >
                <Box sx={modalStyle} width="800px">
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
      </MenuContext.Provider>
    </SettingComp>
  );
}
