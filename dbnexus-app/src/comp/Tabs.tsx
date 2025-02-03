import React, { ReactNode, useContext } from 'react';
import { Stack, Button, Paper, MenuItem, MenuList } from '@mui/material';

import { ContextMenuContext, VisiableContext } from './Common';
import { calMenuStyle } from './helper';
class TabContent {
  constructor(
    public id: string,
    public header: ReactNode,
    public content: ReactNode
  ) {}
}

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
    <Paper>
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
        <MenuItem
          onClick={() => {
            setTabs([]);
            changeTab(0);
          }}
        >
          close all
        </MenuItem>
      </MenuList>
    </Paper>
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
    <Stack spacing={2}>
      <Stack
        direction="row"
        style={{
          flexWrap: 'wrap',
        }}
      >
        {tabs.map((tab, index) => {
          return (
            <Button
              key={tab.id}
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
              variant={index == selectIndex ? 'contained' : 'text'}
              onClick={() => {
                changeTab(index);
              }}
            >
              {tab.header}
            </Button>
          );
        })}
      </Stack>

      <div>
        {tabs.map((tab, index) => (
          <VisiableContext.Provider key={tab.id} value={selectIndex == index}>
            <div style={{ width: '100%' }} hidden={selectIndex !== index}>
              {tab.content}
            </div>
          </VisiableContext.Provider>
        ))}
      </div>
    </Stack>
  );
}
const TabsWithContent = React.memo(TabsWithContent_);
export { TabsWithContent, TabContent };
