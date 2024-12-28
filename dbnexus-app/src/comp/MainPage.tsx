// import Box from '@mui/material/Box';

import { darkTheme } from './theme';
// import { useState } from 'react';

import { NotificationProvider } from './Notification';
// import { createTheme, ThemeProvider } from '@mui/material';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import {
  Box,
  createTheme,
  CssBaseline,
  Modal,
  Stack,
  ThemeProvider,
} from '@mui/material';
import { CSSProperties, ReactNode, useEffect, useState } from 'react';

import { TabContent, TabsWithCloseButton } from './Tabs';
import { WelCome } from './WelCome';
import { MenuContext, modalStyle, TabsSetContext } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';
import AddConfig from './AddConnection.tsx';
import { ModifySettings, SettingComp } from './Settings.tsx';
import types from './ApiStruct.ts';

export default function MainPage() {
  const [showAddConnection, setShowAddConnection] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  listen('add_connection', () => {
    setShowAddConnection(true);
  });

  listen('show_settings', () => {
    setShowSettings(true);
  });

  let [tabs, setTabs] = useState([
    new TabContent('welcome', 'welcome', <WelCome></WelCome>),
  ]);
  const [selectIndex, setSelectIndex] = useState(0);
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
      <TabsSetContext.Provider value={{ setTabs, tabs, setSelectIndex }}>
        <MenuContext.Provider value={{ setUpMenu }}>
          <NotificationProvider>
            <div>
              <SplitPane
                split='vertical'
                minSize={0}
                defaultSize={300}
                style={{ minHeight: '100vh', height: 'auto' }}
              >
                <Left></Left>
                <TabsWithCloseButton
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
                ></TabsWithCloseButton>
              </SplitPane>
            </div>

            <div id='left-side-menu' style={menu && menu.style}>
              {menu && menu.content}
            </div>
            {showAddConnection && (
              <Modal
                open={showAddConnection}
                onClose={() => {
                  setShowAddConnection(false);
                }}
                aria-labelledby='add column'
              >
                <Box sx={modalStyle} width='800px'>
                  <AddConfig
                    handleClose={() => {
                      setShowAddConnection(false);
                    }}
                  ></AddConfig>
                </Box>
              </Modal>
            )}
            {showSettings && (
              <Modal
                open={showSettings}
                onClose={() => {
                  setShowSettings(false);
                }}
                aria-labelledby='add column'
              >
                <Box sx={modalStyle} width='700px'>
                  <ModifySettings
                    handleClose={() => {
                      setShowSettings(false);
                    }}
                  ></ModifySettings>
                </Box>
              </Modal>
            )}
          </NotificationProvider>
        </MenuContext.Provider>
      </TabsSetContext.Provider>
    </SettingComp>
  );
}
