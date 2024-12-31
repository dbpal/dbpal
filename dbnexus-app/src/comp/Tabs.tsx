import { ReactNode, useState } from 'react';
import { Box, Stack, Button } from '@mui/material';

import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
class TabContent {
  constructor(
    public id: string,
    public header: ReactNode,
    public content: ReactNode
  ) {}
}

function TabsWithCloseButton(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: Number) => void;
  selectIndex: number;
}) {
  let { tabs, handleClose, selectIndex, changeTab } = props;
  const [show, setShow] = useState(true);
  function TabPanel(props) {
    const { children, value, id, ...other } = props;
    return (
      <div
        role='tabpanel'
        hidden={value !== id}
        id={`tabpanel-${id}`}
        aria-labelledby={`tab-${id}`}
        {...other}
      >
        {value === id && <Box p={3}>{children}</Box>}
      </div>
    );
  }

  return (
    <div>
      {tabs.length > 0 && (
        <Stack>
          <Button
            style={{ padding: 2 }}
            onClick={() => {
              setShow((e) => !e);
            }}
          >
            {show ? (
              <KeyboardArrowDownIcon fontSize='small'> </KeyboardArrowDownIcon>
            ) : (
              <KeyboardArrowRightIcon fontSize='small'></KeyboardArrowRightIcon>
            )}
          </Button>
        </Stack>
      )}
      {show && (
        <Stack
          direction='row'
          style={{
            flexWrap: 'wrap',
          }}
        >
          {tabs.map((tab, index) => {
            return (
              <Button
                key={tab.id}
                draggable
                onDragEnd={() => {
                  handleClose(index);
                }}
                size='small'
                style={{ padding: '' }}
                //@ts-ignore
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
        <TabPanel key={tab.id} value={selectIndex} id={index}>
          {tab.content}
        </TabPanel>
      ))}
    </div>
  );
}

export { TabsWithCloseButton, TabContent };
