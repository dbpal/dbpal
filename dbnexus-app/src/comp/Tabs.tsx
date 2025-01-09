import { ReactNode, useContext, useState } from 'react';
import { Stack, Button, Paper, MenuItem, MenuList } from '@mui/material';

import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import { calMenuStyle, MenuContext, VisiableContext } from './Common';
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
      </MenuList>

      <MenuItem
        onClick={() => {
          setTabs([]);
          changeTab(0);
        }}
      >
        close all
      </MenuItem>
    </Paper>
  );
}

function TabsWithContent(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: Number) => void;
  selectIndex: number;
  setTabs;
}) {
  let { tabs, handleClose, selectIndex, changeTab, setTabs } = props;
  const [show, setShow] = useState(true);
  const { setUpMenu } = useContext(MenuContext);
  return (
    <Stack spacing={1}>
      {tabs.length > 0 && (
        <Stack>
          <Button
            style={{ padding: 2 }}
            onClick={() => {
              setShow((e) => !e);
            }}
          >
            {show ? (
              <KeyboardArrowDownIcon fontSize="small"> </KeyboardArrowDownIcon>
            ) : (
              <KeyboardArrowRightIcon fontSize="small"></KeyboardArrowRightIcon>
            )}
          </Button>
        </Stack>
      )}
      {show && (
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
                sx={{ textTransform: 'none' }}
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
                variant={index == selectIndex ? 'outlined' : 'text'}
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
      {tabs.map((tab, index) => (
        <VisiableContext.Provider key={tab.id} value={selectIndex == index}>
          <div style={{ width: '100%' }} hidden={selectIndex !== index}>
            {tab.content}
          </div>
        </VisiableContext.Provider>
      ))}
    </Stack>
  );
}

export { TabsWithContent, TabContent };
