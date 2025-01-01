import { ReactNode, useState } from 'react';
import { Stack, Button } from '@mui/material';

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
                size='small'
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
        <div key={tab.id} hidden={selectIndex !== index}>
          {tab.content}
        </div>
      ))}
    </Stack>
  );
}

export { TabsWithCloseButton, TabContent };
