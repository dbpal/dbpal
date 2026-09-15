import { useRef } from "react";
import {
  Stack,
  MenuItem,
  MenuList,
  Tabs,
  Tab,
  Tooltip,
  Chip,
  useTheme,
  Divider,
} from "@mui/material";
import { CloseIcon } from "./icons";
import {
  secondConfirm,
  TabContext,
  TabActiveContext,
  DbnameContext,
  TabContent,
  extractConfigNameFromUuid,
} from "./helper";

import {
  useBoolSwitch,
  useConfirmMsg,
  useConnected,
  useContextMenu,
  useTabsManage,
} from "./store";
import { IconModifed } from "./CommonComp2";
import React from "react";

import { AiOutlineDisconnect } from "./icons";
const tabsWrapKey = "tabs-wrap-key";
function TabsMenu(props: { index: number }) {
  const { index } = props;
  const { closeTab, setTabs, setCurrentTab, tabs } = useTabsManage();
  const tabsLength = tabs.length;

  const { switches, setSwitch } = useBoolSwitch();

  const wrap = switches[tabsWrapKey];

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

      <MenuItem
        onClick={() => {
          setSwitch(tabsWrapKey, !wrap);
        }}>
        {wrap ? "nowrap" : "wrap"}
      </MenuItem>
    </MenuList>
  );
}

function TabsWithContent() {
  const { tabs, setCurrentTab, currentTab, closeTab } = useTabsManage();
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const rootStack = useRef<HTMLDivElement>(null);
  const tabConfirmMsg = useConfirmMsg((state) => state.confirmMsgs);
  const { connected } = useConnected();
  const { switches } = useBoolSwitch();
  const wrap = switches[tabsWrapKey];
  const theme = useTheme();
  const getIcon = (tab: TabContent) => {
    if (tab.dbName) {
      const name = extractConfigNameFromUuid(tab.dbName);
      if (connected.has(name)) {
        return tab.icon;
      } else {
        return <AiOutlineDisconnect style={{ fontSize: "1.2rem" }} color="#7ef301" />;
      }
    } else {
      return tab.icon;
    }
  };
  return (
    <Stack style={{ width: "100%", height: "100%" }} ref={rootStack}>
      {!wrap && (
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
            return (
              <Tab
                style={{
                  maxWidth:
                    (function () {
                      const max = (100 * 2) / tabs.length;
                      return max;
                    })() + "%",
                }}
                key={tab.id}
                label=<Tooltip title={tab.header}>
                  <Chip
                    color={currentTab == index ? "primary" : "default"}
                    icon={getIcon(tab)}
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
      )}
      {wrap && (
        <Stack
          direction="row"
          style={{ alignItems: "center", flexWrap: "wrap" }}
          spacing={0}>
          {tabs.map((tab, index) => (
            <div
              key={tab.id}
              style={{
                paddingBottom: "0.7rem",
                borderBottom:
                  index == currentTab
                    ? "2px solid " + theme.palette.primary.main
                    : undefined,
              }}>
              <Chip
                color={currentTab == index ? "primary" : "default"}
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
                icon={getIcon(tab)}
                sx={{
                  marginLeft: "4px",
                  marginTop: "2px",
                  cursor: "pointer",
                }}
                onDelete={() => {
                  closeTab(index);
                }}
                onClick={() => {
                  setCurrentTab(index);
                }}
                label={tab.header}
                onContextMenu={(e) => {
                  setUpMenu({
                    content: <TabsMenu index={index} />,
                    event: e,
                  });
                }}
              />
            </div>
          ))}
        </Stack>
      )}

      {tabs.length > 0 && <Divider />}

      <div style={{ flex: 1 }}>
        {tabs.map((tab, index) => (
          <TabItem key={tab.id} tab={tab} isActive={currentTab === index} />
        ))}
      </div>
    </Stack>
  );
}

// 提取为独立组件
// eslint-disable-next-line react/display-name
const TabItem = React.memo(
  ({ tab, isActive }: { tab: TabContent; isActive: boolean }) => {
    return (
      <TabActiveContext.Provider value={isActive}>
        <TabContext value={{ id: tab.id }}>
          <DbnameContext value={{ dbName: tab.dbName, kind: tab.kind }}>
            <div
              style={{
                height: "100%",
                position: "relative",
                display: isActive ? "block" : "none", // 用 display 替代 hidden
              }}>
              {tab.content}
            </div>
          </DbnameContext>
        </TabContext>
      </TabActiveContext.Provider>
    );
  },
);

export { TabsWithContent };
