import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));

import React, { useEffect, useState } from 'react';
import { Stack, Button } from '@mui/material';

import { generateRandomString } from './helper';
import { TabContent, TabsWithCloseButton } from './Tabs';

function TestTabsWithCloseButton() {
  let [tabs, setTabs] = useState<TabContent[]>([]);
  useEffect(() => {}, [tabs]);
  return (
    <Stack>
      <Button
        onClick={() => {
          setTabs((e) => {
            return [
              ...e,
              new TabContent(
                generateRandomString(10),
                generateRandomString(20),
                generateRandomString(20)
              ),
            ];
          });
        }}
      >
        Add
      </Button>
      <TabsWithCloseButton
        handleClose={(id: string) => {
          setTabs((prevTabs) => prevTabs.filter((x) => x.id !== id));
        }}
        tabs={tabs}
      ></TabsWithCloseButton>
    </Stack>
  );
}

root.render(
  <React.StrictMode>
    <TestTabsWithCloseButton></TestTabsWithCloseButton>
  </React.StrictMode>
);
