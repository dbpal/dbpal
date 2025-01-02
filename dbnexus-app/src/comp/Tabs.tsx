import { ReactNode, useState } from 'react';
import { Stack, Button } from '@mui/material';

import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import { VisiableContext } from './Common';
class TabContent {
  constructor(
    public id: string,
    public header: ReactNode,
    public content: ReactNode
  ) {}
}

function TabsWithContent(props: {
  tabs: TabContent[];
  handleClose: (index: number) => void;
  changeTab: (index: Number) => void;
  selectIndex: number;
}) {
  let { tabs, handleClose, selectIndex, changeTab } = props;
  const [show, setShow] = useState(true);

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
                sx={{ textTransform: 'none' }}
                onDragEnd={() => {
                  handleClose(index);
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
          <div hidden={selectIndex !== index}>{tab.content}</div>
        </VisiableContext.Provider>
      ))}
    </Stack>
  );
}

export { TabsWithContent, TabContent };
