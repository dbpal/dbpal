import React, { useContext, useEffect, useRef, useState } from "react";
import {
  Stack,
  MenuItem,
  MenuList,
  Tabs,
  Tab,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";

import {
  ContextMenuContext,
  secondConfirm,
  SetCloseConfirmTextContext,
  TabContent,
  TabActiveContext,
  WindowFocusContext,
} from "./helper";

function TabsMenu(props: {
  index;
  handleClose;
  setTabs;
  changeTab;
  tabsLength;
  selectIndex;
}) {
  const { index, handleClose, setTabs, changeTab, tabsLength, selectIndex } = props;

  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          handleClose(index);
        }}
      >
        close
      </MenuItem>
      {index != 0 && (
        <MenuItem
          onClick={() => {
            setTabs(e => {
              changeTab(0);
              return e.slice(index);
            });
          }}
        >
          close to left
        </MenuItem>
      )}
      {index != tabsLength - 1 && (
        <MenuItem
          onClick={() => {
            setTabs(e => {
              const x = e.slice(0, index + 1);
              if (selectIndex > index) {
                changeTab(index);
              }
              return x;
            });
          }}
        >
          close to right
        </MenuItem>
      )}
      {tabsLength > 1 && (
        <MenuItem
          onClick={() => {
            setTabs(e => {
              const x = [e[selectIndex]];
              changeTab(0);
              return x;
            });
          }}
        >
          close others
        </MenuItem>
      )}
      {tabsLength > 1 && (
        <MenuItem
          onClick={async () => {
            const confirm = await secondConfirm({
              title: "close all tab",
              confirmText: "Close",
              kind: "error",
            });
            if (!confirm) {
              return;
            }
            setTabs([]);
            changeTab(0);
          }}
        >
          close all
        </MenuItem>
      )}
    </MenuList>
  );
}

function getLength(header: string, width: number, tabs: TabContent[]) {
  let total: number = tabs.reduce((acc, cur) => {
    return acc + cur.header.length;
  }, 0);
  return (width * header.length * 1.5) / total;
}

function TabsWithContent_(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: number) => void;
  selectIndex: number;
  setTabs;
}) {
  const { tabs, handleClose, selectIndex, changeTab, setTabs } = props;
  const { setUpMenu } = useContext(ContextMenuContext);
  const windowFocus = useContext(WindowFocusContext);
  const rootStack = useRef<HTMLDivElement>(null);
  const [tabWidth, setTabWidth] = useState(100);
  useEffect(() => {
    if (tabs.length == 0) {
      return;
    }
    const e = new ResizeObserver(en => {
      const width = en[0].target.clientWidth;
      if (width) {
        setTabWidth(width);
      }
    });
    e.observe(rootStack.current);
    return () => {
      e.disconnect();
    };
  }, [tabs]);

  return (
    <Stack style={{ width: "100%", height: "100%" }} ref={rootStack}>
      <Tabs
        value={selectIndex}
        variant="scrollable"
        scrollButtons="auto"
        allowScrollButtonsMobile
        onChange={(_, newValue) => {
          changeTab(newValue);
        }}
        sx={{
          // 1. Tab 之间缝隙
          "& .MuiTabs-flexContainer": { columnGap: 0 }, // 4px
          // 2. Tab 内边距
          "& .MuiTab-root": { minHeight: 36, px: 0.4 }, // 高度36px，左右12px
          opacity: windowFocus ? 1 : 0.6,
        }}
      >
        {tabs.map((tab, index) => {
          const max = getLength(tab.header, tabWidth, tabs);
          return (
            <Tab
              style={{ maxWidth: max + "px" }}
              key={tab.id}
              label=<Tooltip title={tab.header}>
                <Stack
                  direction="row"
                  style={{ textTransform: "none", alignItems: "center" }}
                  onContextMenu={e => {
                    setUpMenu({
                      content: (
                        <TabsMenu
                          index={index}
                          handleClose={handleClose}
                          setTabs={setTabs}
                          tabsLength={tabs.length}
                          changeTab={changeTab}
                          selectIndex={selectIndex}
                        />
                      ),
                      event: e,
                    });
                  }}
                >
                  {tabs.length <= 8 ? tab.icon : undefined}
                  <Typography
                    style={{
                      textOverflow: "clip",
                      overflow: "hidden",
                      whiteSpace: "nowrap",
                      display: "inline-block",
                      fontWeight: index == selectIndex ? "bolder" : undefined,
                    }}
                    color={index == selectIndex ? "primary" : undefined}
                  >
                    {tab.header}
                  </Typography>
                  <CloseIcon
                    onClick={() => {
                      handleClose(index);
                    }}
                    color="warning"
                  />
                </Stack>
              </Tooltip>
            />
          );
        })}
      </Tabs>
      <div style={{ flex: 1 }}>
        {tabs.map((tab, index) => (
          <TabActiveContext.Provider key={tab.id} value={selectIndex == index}>
            <SetCloseConfirmTextContext
              value={e => {
                tabs[index].closeConfirmText = e;
              }}
            >
              <div
                hidden={selectIndex !== index}
                style={{ height: "100%", position: "relative" }}
              >
                {tab.content}
              </div>
            </SetCloseConfirmTextContext>
          </TabActiveContext.Provider>
        ))}
      </div>
    </Stack>
  );
}
const TabsWithContent = React.memo(TabsWithContent_);
export { TabsWithContent, TabContent };
