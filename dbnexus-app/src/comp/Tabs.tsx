import { useEffect, useRef, useState } from "react";
import { Stack, MenuItem, MenuList, Tabs, Tab, Tooltip, Chip } from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { secondConfirm, TabContext, TabActiveContext } from "./helper";

import { useConfirmMsg, useContextMenu, useTabsManage } from "./store";
import { IconModifed } from "./CommonComp2";

function TabsMenu(props: { index: number }) {
  const { index } = props;
  const { closeTab, setTabs, setCurrentTab, tabs } = useTabsManage();
  const tabsLength = tabs.length;
  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          closeTab(index);
        }}>
        close
      </MenuItem>
      {index != 0 && (
        <MenuItem
          onClick={() => {
            setCurrentTab(0);
            setTabs(tabs.slice(index));
          }}>
          close to left
        </MenuItem>
      )}
      {index != tabsLength - 1 && (
        <MenuItem
          onClick={() => {
            setTabs(tabs.slice(0, index + 1));
          }}>
          close to right
        </MenuItem>
      )}
      {tabsLength > 1 && (
        <MenuItem
          onClick={() => {
            setCurrentTab(0);
            setTabs([tabs[index]]);
          }}>
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
            setCurrentTab(0);
          }}>
          close all
        </MenuItem>
      )}
    </MenuList>
  );
}

function TabsWithContent() {
  const { tabs, setCurrentTab, currentTab, closeTab } = useTabsManage();
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
        value={currentTab}
        variant="scrollable"
        scrollButtons="auto"
        allowScrollButtonsMobile
        onChange={(_, newValue) => {
          setCurrentTab(newValue);
        }}
        sx={{
          // 1. Tab 之间缝隙
          "& .MuiTabs-flexContainer": { columnGap: 0 }, // 4px
          // 2. Tab 内边距
          "& .MuiTab-root": { minHeight: 36, px: 0.4 }, // 高度36px，左右12px
        }}>
        {tabs.map((tab, index) => {
          const max = (tabWidth * 2.2) / tabs.length;
          return (
            <Tab
              style={{ maxWidth: max + "px" }}
              key={tab.id}
              label=<Tooltip title={tab.header}>
                <Chip
                  color={currentTab == index ? "success" : "default"}
                  icon={tab.icon}
                  style={{
                    textTransform: "none",
                    alignItems: "center",
                    fontWeight: currentTab == index ? "bolder" : undefined,
                  }}
                  size="largeSmall"
                  deleteIcon={
                    tabConfirmMsg.get(tab.id) ? (
                      <IconModifed />
                    ) : (
                      <CloseIcon
                        onClick={() => {
                          closeTab(index);
                        }}
                        style={{ fontSize: "0.9rem" }}
                        color="warning"
                      />
                    )
                  }
                  onDelete={() => {
                    closeTab(index);
                  }}
                  label={tab.header}
                  onContextMenu={(e) => {
                    setUpMenu({
                      content: <TabsMenu index={index} />,
                      event: e,
                    });
                  }}
                />
              </Tooltip>
            />
          );
        })}
      </Tabs>
      <div style={{ flex: 1 }}>
        {tabs.map((tab, index) => (
          <TabActiveContext.Provider key={tab.id} value={currentTab == index}>
            <TabContext value={{ id: tab.id }}>
              <div
                hidden={currentTab !== index}
                style={{ height: "100%", position: "relative" }}>
                {tab.content}
              </div>
            </TabContext>
          </TabActiveContext.Provider>
        ))}
      </div>
    </Stack>
  );
}

export { TabsWithContent };
