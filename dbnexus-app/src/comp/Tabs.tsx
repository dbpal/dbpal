import React, { useContext, useState } from 'react';
import { Stack, Button, Paper, MenuItem, MenuList } from '@mui/material';

import { VisiableContext } from './Common';
import {
  calMenuStyle,
  ContextMenuContext,
  secondConfirm,
  TabContent,
} from './helper';

function TabsMenu(props: {
  index;
  handleClose;
  setTabs;
  changeTab;
  tabsLength;
  selectIndex;
  setHideAll;
}) {
  const {
    index,
    handleClose,
    setTabs,
    changeTab,
    tabsLength,
    selectIndex,
    setHideAll,
  } = props;

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
          onClick={async () => {
            const confirm = await secondConfirm({
              title: 'close all tab',
              content: 'are u sure',
              confirmText: 'close',
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

        <MenuItem
          onClick={() => {
            setHideAll(true);
          }}
        >
          hide all
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
  const [hideAll, setHideAll] = useState(false);
  return (
    <Stack spacing={1}>
      {hideAll && (
        <Stack>
          <Button
            onClick={() => {
              setHideAll(false);
            }}
            style={{ padding: 0, margin: 0 }}
          >
            all tabs
          </Button>
        </Stack>
      )}

      {!hideAll && (
        <Stack
          direction="row"
          style={{
            flexWrap: 'wrap',
          }}
          spacing={1}
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
                        setHideAll={setHideAll}
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
      )}

      <div style={{ width: '98.7%' }}>
        {tabs.map((tab, index) => (
          <VisiableContext.Provider key={tab.id} value={selectIndex == index}>
            <div hidden={selectIndex !== index}>{tab.content}</div>
          </VisiableContext.Provider>
        ))}
      </div>
    </Stack>
  );
}
const TabsWithContent = React.memo(TabsWithContent_);
export { TabsWithContent, TabContent };
