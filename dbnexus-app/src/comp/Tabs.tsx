import { ReactNode } from 'react';
import { Tabs, Tab, Box, IconButton, Stack } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

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
      <Tabs
        value={selectIndex}
        onChange={(_e, index) => {
          changeTab(index);
        }}
      >
        {tabs.map((tab, index) => (
          <Tab
            key={index}
            label=<Stack direction='row'>
              {tab.header}
              <CloseIcon onClick={() => handleClose(index)} fontSize='small' />
            </Stack>
          />
        ))}
      </Tabs>

      {tabs.map((tab, index) => (
        <TabPanel key={index} value={selectIndex} id={index}>
          {tab.content}
        </TabPanel>
      ))}
    </div>
  );
}

export { TabsWithCloseButton, TabContent };
