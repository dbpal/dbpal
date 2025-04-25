import { NotificationProvider } from './Notification';

import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';

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
import { tabsHasId } from './Common.tsx';
import { Left } from './Left';
import { listen } from '@tauri-apps/api/event';

import { ModifySettings, SettingComp } from './Settings.tsx';
import {
  appStore,
  AppStoreKey,
  ContextMenuContext,
  TabsSetContext,
} from './helper.ts';
import { IoMdSettings } from 'react-icons/io';
export default function MainPage() {
  let [tabs, setTabs] = useState([
    new TabContent(
      'welcome',
      'welcome',
      <WelCome></WelCome>,
      <SiWelcometothejungle> </SiWelcometothejungle>
    ),
  ]);
  const [selectTab, setSelectTab] = useState(0);

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
            new TabContent(
              id,
              id,
              <ModifySettings></ModifySettings>,
              <IoMdSettings> </IoMdSettings>
            ),
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
    if (selectTab > 0 && selectTab >= tabs.length) {
      setSelectTab((e) => e - 1);
    }
  }, [tabs]);

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
        selectIndex={selectTab}
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
          </NotificationProvider>
        </TabsSetContext.Provider>
      </ContextMenuContext.Provider>
    </SettingComp>
  );
}
