import { Button, Stack, Typography } from '@mui/material';
import { useContext, useEffect, useState } from 'react';

import { appStore, JsonError, validJson } from './helper';
import { NotificationContext } from './Notification';
import { SimpleFlexSizeEditor } from './Common';

function EditConfigFile(props: { handleClose }) {
  const { handleClose } = props;

  const [config, setConfig] = useState('');
  const [jsonError, setJsonError] = useState<JsonError>(null);

  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    getAll()
      .then((e) => {
        let jsonStr = JSON.stringify(e, null, 2);
        setConfig(jsonStr);
      })
      .catch((e) => {
        showNotification('error', 'Error', 'Failed to load config file: ' + e);
      });
  }, []);

  useEffect(() => {
    validJson(config)
      .then(() => {
        setJsonError(null);
      })
      .catch((e) => {
        setJsonError(e);
      });
  }, [config]);

  const handleSave = async () => {
    if (jsonError) {
      return;
    }
    try {
      const parsed = JSON.parse(config);
      await appStore.clear();
      for (const key in parsed) {
        await appStore.set(key, parsed[key]);
      }
      showNotification('success', 'Success', 'Config file saved successfully');
      handleClose();
    } catch (e) {
      showNotification('error', 'Error', 'Failed to save config file: ' + e);
    }
  };
  return (
    <Stack spacing={1} flex={1}>
      <SimpleFlexSizeEditor
        value={config}
        handleValue={(e) => {
          setConfig(e);
        }}
        language="json"
        barLeft={
          jsonError ? (
            <Typography color="error"> {jsonError.msg}</Typography>
          ) : undefined
        }
      ></SimpleFlexSizeEditor>
      <Stack direction="row">
        <Button variant="contained" onClick={handleSave}>
          save
        </Button>
      </Stack>
    </Stack>
  );
}

async function getAll() {
  let x = {};
  let keys = await appStore.keys();

  for (let key of keys) {
    let value = await appStore.get(key);
    x[key] = value;
  }
  return x;
}
export { EditConfigFile };
