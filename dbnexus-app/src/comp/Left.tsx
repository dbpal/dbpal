import AddIcon from '@mui/icons-material/Add';
import { Button, Stack } from '@mui/material';
import { useContext } from 'react';
import { TabContent } from './Tabs';
import { tabsHasId, TabsSetContext } from './Common';
import AddConfig from './AddConnection';

function Left() {
  let { setTabs, tabs, setSelectIndex } = useContext(TabsSetContext);

  return (
    <Stack>
      <Stack direction='row'>
        <Button
          color='primary'
          variant='contained'
          onClick={() => {
            const id = 'add_config';
            let index = tabsHasId(tabs, 'add_config');
            if (typeof index != 'number') {
              setTabs((e) => {
                setSelectIndex(tabs.length);
                return [
                  ...e,
                  new TabContent(id, 'add config', <AddConfig></AddConfig>),
                ];
              });
            } else {
              setSelectIndex(index);
            }
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>
    </Stack>
  );
}

export { Left };
