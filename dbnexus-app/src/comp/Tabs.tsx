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
  calMenuStyle,
  ContextMenuContext,
  secondConfirm,
  TabContent,
  VisiableContext,
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
              let x = e.slice(0, index + 1);
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
              let x = [e[selectIndex]];
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
              content: 'are u sure',
              confirmText: 'Close',
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
  let { tabs, handleClose, selectIndex, changeTab, setTabs } = props;
  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <Stack spacing={1} style={{ width: '100%' }}>
      <div style={{ display: 'grid' }}>
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
          }}
        >
          {tabs.map((tab, index) => {
            return (
              <Tab
                key={tab.id}
                label=<Tooltip title={tab.header} arrow>
                  <Chip
                    label={tab.header}
                    onDelete={() => {
                      handleClose(index);
                    }}
                    style={{ padding: '0px', maxWidth: '14rem' }}
                    icon={tab.icon}
                    size="small"
                    color={index == selectIndex ? 'primary' : 'default'}
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
                        style: calMenuStyle(e, 100),
                      });
                    }}
                  ></Chip>
                </Tooltip>
              ></Tab>
            );
          })}
        </Tabs>
      </div>
      <div
        style={{
          paddingRight: '1.8rem',
        }}
      >
        {tabs.map((tab, index) => (
          <VisiableContext.Provider key={tab.id} value={selectIndex == index}>
            <div hidden={selectIndex !== index}>{tab.content}</div>
          </VisiableContext.Provider>
        ))}
      </div>

      <div style={{ minHeight: '300px' }}></div>
    </Stack>
  );
}
const TabsWithContent = React.memo(TabsWithContent_);
export { TabsWithContent, TabContent };
