// import Box from '@mui/material/Box';

import { darkTheme } from './theme';
// import { useState } from 'react';

import { NotificationProvider } from './Notification';
// import { createTheme, ThemeProvider } from '@mui/material';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';
import { createTheme, ThemeProvider } from '@mui/material';
import { ReactNode, useEffect, useState } from 'react';

import { TabContent, TabsWithCloseButton } from './Tabs';
import { WelCome } from './WelCome';
import { MenuContext, TabsSetContext } from './Common.tsx';
import { Left } from './Left';

export default function MainPage() {
  const lightTheme = createTheme();
  const [theme, setTheme] = useState('light'); // 默认为明亮主题

  let [tabs, setTabs] = useState([
    new TabContent('welcome', 'welcome', <WelCome></WelCome>),
  ]);
  const [selectIndex, setSelectIndex] = useState(0);
  const [menu, setMenuContent] = useState<ReactNode>(null);
  useEffect(() => {
    // cancel the menu
    const customMenu = document.getElementById('left-side-menu');
    window.addEventListener('click', () => {
      customMenu.style.display = 'none';
    });
  });

  return (
    <ThemeProvider theme={theme === 'light' ? lightTheme : darkTheme}>
      <TabsSetContext.Provider value={{ setTabs, tabs, setSelectIndex }}>
        <MenuContext.Provider value={{ setMenuContent }}>
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
            <div id='left-side-menu'>{menu}</div>
          </NotificationProvider>
        </MenuContext.Provider>
      </TabsSetContext.Provider>
    </ThemeProvider>
  );
}
