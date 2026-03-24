import { useEffect, useRef, useState } from "react";
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
import { secondConfirm, TabContext, TabContent, TabActiveContext } from "./helper";

import CircleIcon from "@mui/icons-material/Circle";
import { useConfirmMsg, useContextMenu } from "./store";

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
            setTabs((e) => {
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
            setTabs((e) => {
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
            setTabs((e) => {
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

function TabsWithContent(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: number) => void;
  selectIndex: number;
  setTabs;
}) {
  const { tabs, handleClose, selectIndex, changeTab, setTabs } = props;
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const rootStack = useRef<HTMLDivElement>(null);
  const [tabWidth, setTabWidth] = useState(100);

  const tabConfirmMsg = useConfirmMsg((state) => state.confirmMsgs);

  useEffect(() => {
    if (tabs.length == 0) {
      return;
    }
    const e = new ResizeObserver((en) => {
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
        }}
      >
        {tabs.map((tab, index) => {
          const max = getLength(tab.header, tabWidth, tabs);
          return (
            <Tab
              style={{ maxWidth: index == selectIndex ? undefined : max + "px" }}
              key={tab.id}
              label=<Tooltip title={tab.header}>
                <Stack
                  direction="row"
                  style={{
                    textTransform: "none",
                    alignItems: "center",
                  }}
                  onContextMenu={(e) => {
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
                  {tabs.length <= 8 && max > 100 ? tab.icon : undefined}
                  <Typography
                    style={{
                      fontWeight: index == selectIndex ? "bolder" : undefined,
                      minWidth: "2rem",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "clip",
                      flex: 1,
                      fontSize: index == selectIndex ? "1.1rem" : "1rem",
                    }}
                  >
                    {tab.header}
                  </Typography>

                  {tabConfirmMsg.get(tab.id) ? (
                    <CircleIcon color="warning" style={{ fontSize: "0.9rem" }} />
                  ) : (
                    <CloseIcon
                      onClick={() => {
                        handleClose(index);
                      }}
                      style={{ fontSize: "0.9rem" }}
                      color="warning"
                    />
                  )}
                </Stack>
              </Tooltip>
            />
          );
        })}
      </Tabs>
      <div style={{ flex: 1 }}>
        {tabs.map((tab, index) => (
          <TabActiveContext.Provider key={tab.id} value={selectIndex == index}>
            <TabContext value={{ id: tab.id }}>
              <div
                hidden={selectIndex !== index}
                style={{ height: "100%", position: "relative" }}
              >
                {tab.content}
              </div>
            </TabContext>
          </TabActiveContext.Provider>
        ))}
      </div>
    </Stack>
  );
}

export { TabsWithContent, TabContent };
