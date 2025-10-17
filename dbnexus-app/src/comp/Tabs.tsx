'use strict';
import React, { useContext } from 'react';
import {
  Stack,
  MenuItem,
  MenuList,
  Chip,
  Tabs,
  Tab,
  Tooltip,
} from '@mui/material';

import {
  ContextMenuContext,
  secondConfirm,
  SetCloseConfirmTextContext,
  TabContent,
  TabActiveContext,
  WindowFocusContext,
  SettingContext,
} from './helper';

function TabsMenu(props: {
  index;
  handleClose;
  setTabs;
  changeTab;
  tabsLength;
  selectIndex;
}) {
  const { index, handleClose, setTabs, changeTab, tabsLength, selectIndex } =
    props;

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
              title: 'close all tab',
              confirmText: 'Close',
              kind: 'error',
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

function TabsWithContent_(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: Number) => void;
  selectIndex: number;
  setTabs;
}) {
  const { tabs, handleClose, selectIndex, changeTab, setTabs } = props;
  const { setUpMenu } = useContext(ContextMenuContext);
  const windowFocus = useContext(WindowFocusContext);
  const setting = useContext(SettingContext);
  return (
    <Stack spacing={1} style={{ width: '100%', height: '100%' }}>
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
          '& .MuiTabs-flexContainer': { columnGap: 0 }, // 4px
          // 2. Tab 内边距
          '& .MuiTab-root': { minHeight: 36, px: 0.4 }, // 高度36px，左右12px
          opacity: windowFocus ? 1 : 0.6,
        }}
      >
        {tabs.map((tab, index) => {
          return (
            <Tab
              key={tab.id}
              label=<Tooltip title={tab.header}>
                <Chip
                  label={tab.header}
                  onDelete={() => {
                    handleClose(index);
                  }}
                  style={{
                    padding: '0px',
                    maxWidth: setting.appearance.maxTabTitleLength + 'rem',
                  }}
                  icon={tab.icon}
                  variant={index == selectIndex ? 'filled' : 'outlined'}
                  color={index == selectIndex ? 'success' : 'default'}
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
                        ></TabsMenu>
                      ),
                      event: e,
                    });
                  }}
                ></Chip>
              </Tooltip>
            ></Tab>
          );
        })}
      </Tabs>

      <div style={{ flex: 1 }}>
        {tabs.map((tab, index) => (
          <TabActiveContext.Provider key={tab.id} value={selectIndex == index}>
            <SetCloseConfirmTextContext
              value={(e) => {
                tabs[index].closeConfirmText = e;
              }}
            >
              <div hidden={selectIndex !== index} style={{ height: '100%' }}>
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
