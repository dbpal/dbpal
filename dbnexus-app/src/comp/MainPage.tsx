// import Box from '@mui/material/Box';

import { darkTheme } from './theme';
// import { useState } from 'react';
import AddIcon from '@mui/icons-material/Add';
import { NotificationProvider } from './Notification';
// import { createTheme, ThemeProvider } from '@mui/material';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { Button, createTheme, Stack, ThemeProvider } from '@mui/material';
import { createContext, useContext, useState } from 'react';
import { generateRandomString } from './helper';
import { TabContent, TabsWithCloseButton } from './Tabs';
import { WelCome } from './WelCome';
import { TabsSetContext } from './Common.tsx';
import { Left } from './Left';

export default function MainPage() {
  const lightTheme = createTheme();
  const [theme, setTheme] = useState('light'); // 默认为明亮主题
  const isDarkMode = theme === 'dark';
  let [tabs, setTabs] = useState([
    new TabContent('welcome', 'welcome', <WelCome></WelCome>),
  ]);
  const [selectIndex, setSelectIndex] = useState(0);

  return (
    <ThemeProvider theme={theme === 'light' ? lightTheme : darkTheme}>
      <NotificationProvider>
        <SplitPane
          split='vertical'
          minSize={0}
          defaultSize={300}
          style={{ minHeight: '100vh', height: 'auto' }}
        >
          <TabsSetContext.Provider value={{ setTabs, tabs, setSelectIndex }}>
            <Left></Left>
          </TabsSetContext.Provider>

          <TabsSetContext.Provider value={{ setTabs, tabs, setSelectIndex }}>
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
                setSelectIndex(index);
              }}
            ></TabsWithCloseButton>
          </TabsSetContext.Provider>
        </SplitPane>
      </NotificationProvider>
    </ThemeProvider>
  );
}
