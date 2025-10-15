'use strict';
import { NotificationProvider } from './Notification';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { TabContent, TabsWithContent } from './Tabs';
import { Left } from './Left';
import { SettingComp } from './Settings.tsx';
import {
  ContextMenuContext,
  secondConfirm,
  TabsSetContext,
  WindowFocusContext,
  getViewportSize,
  KeyPressed,
} from './helper.ts';
import '../app.css';
import { Paper, useTheme } from '@mui/material';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

const shortCuts = {
  callback: (e: KeyboardEvent) => {
    if (KeyPressed.prevTab(e)) {
      shortCuts.prev();
    }
    if (KeyPressed.nextTab(e)) {
      shortCuts.next();
    }
    if (KeyPressed.closeTab(e)) {
      shortCuts.closeCurrentTab(e);
    }
  },
} as {
  prev: () => void;
  next: () => void;
  callback: (e: KeyboardEvent) => void;
  closeCurrentTab: (_: KeyboardEvent) => void;
};

function App() {
  const [tabs, setTabs] = useState<TabContent[]>([]);
  const [tab, setTab] = useState(0);
  const closeTab = useCallback(
    (index: number) => {
      (async () => {
        const id = tabs[index].id;
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

  shortCuts.closeCurrentTab = (e) => {
    if (tabs.length > 0) {
      closeTab(tab);
      e.preventDefault();
    }
  };

  // shortcut
  useEffect(() => {
    document.addEventListener('keydown', shortCuts.callback);
    return () => {
      document.removeEventListener('keydown', shortCuts.callback);
    };
  }, []);

  useEffect(() => {
    if (tab > 0 && tab >= tabs.length) {
      setTab((e) => e - 1);
    }
  }, [tabs]);

  const changeTab = useCallback((index) => {
    setTab(index as number);
  }, []);

  return (
    <SettingComp>
      <TabsSetContext.Provider value={{ setTabs, tabs, setSelectTab: setTab }}>
        <NotificationProvider>
          <Menu>
            <WindowFocus>
              <div>
                <PanelGroup
                  direction="horizontal"
                  style={{
                    height: '100vh',
                    width: '100vw',
                    padding: '0.5rem 1rem',
                  }}
                  autoSaveId="main"
                >
                  <Panel
                    style={{
                      overflowY: 'auto',
                      overflowX: 'hidden',
                      height: '100%',
                    }}
                    minSize={10}
                    maxSize={65}
                    collapsible
                    collapsedSize={0}
                  >
                    <Left></Left>
                  </Panel>
                  <PanelResizeHandle
                    style={{ width: 4 }}
                    className="resize-handle"
                  />
                  <Panel
                    style={{
                      overflow: 'auto',
                      height: '100%',
                    }}
                  >
                    <TabsWithContent
                      handleClose={closeTab}
                      tabs={tabs}
                      selectIndex={tab}
                      changeTab={changeTab}
                      setTabs={setTabs}
                    ></TabsWithContent>
                  </Panel>
                </PanelGroup>
              </div>
            </WindowFocus>
          </Menu>
        </NotificationProvider>
      </TabsSetContext.Provider>
    </SettingComp>
  );
}

function WindowFocus(props: { children: ReactNode }) {
  const { children } = props;
  const [windowFocus, setWindowsFocus] = useState(true);
  useEffect(() => {
    const webview = WebviewWindow.getCurrent();
    const unlisten = webview.onFocusChanged((e) => {
      setWindowsFocus(e.payload);
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, []);

  return (
    <WindowFocusContext value={windowFocus}>{children}</WindowFocusContext>
  );
}

function Menu(props: { children: ReactNode }) {
  const { children } = props;
  const [menu, setUpMenu] = useState<{
    content: ReactNode;
    event: MouseEvent;
  }>(null);

  const menuDiv = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.addEventListener('click', () => {
      if (!menuDiv.current.style) {
        return;
      }
      menuDiv.current.style.display = 'none';
    });
  }, []);

  useEffect(() => {
    if (!menu) {
      return;
    }
    menuDiv.current.style.display = 'block';
    if (menu && menu.event) {
      menu.event.preventDefault();
    }
  }, [menu]);

  const theme = useTheme();

  return (
    <ContextMenuContext.Provider value={{ setUpMenu }}>
      {children}
      <div
        ref={menuDiv}
        style={{
          position: 'fixed',
          zIndex: 10000,
          height: 'fit-content',
          maxHeight: '48vh',
          overflow: 'auto',
          display: 'block',
          border: '2px solid ' + theme.palette.success.main,
          borderRadius: '0.5rem',
          ...(menu && calMenuStyle(menu.event)),
        }}
      >
        <Paper elevation={24}>{menu && menu.content}</Paper>
      </div>
    </ContextMenuContext.Provider>
  );
}

function SeparateWindow(props: { children }) {
  const { children } = props;

  return (
    <SettingComp>
      <Menu>
        <NotificationProvider>
          <div
            style={{
              padding: '0.5rem 1rem',
              width: '100vw',
              height: '100vh',
            }}
          >
            {children}
          </div>
        </NotificationProvider>
      </Menu>
    </SettingComp>
  );
}

function calMenuStyle(e: MouseEvent): CSSProperties {
  const x = e.clientX;
  const y = e.clientY;
  let ret: CSSProperties = {};
  const { height, width } = getViewportSize();
  ret.top = 'auto';
  ret.bottom = 'auto';
  ret.left = 'auto';
  ret.right = 'auto';
  if (width - x > width / 2) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      'px';
  } else {
    ret.right = width - x - 10 + 'px';
  }

  if (height - y > height / 2) {
    // 这个是点击的位置下面有很大的空间
    ret.top = y - 10 + 'px';
  } else {
    ret.bottom = height - y - 10 + 'px';
  }
  return ret;
}

export { App, SeparateWindow };
