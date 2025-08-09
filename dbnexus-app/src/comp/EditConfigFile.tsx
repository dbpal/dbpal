import { Button, Stack, Typography } from '@mui/material';
import { useContext, useEffect, useState } from 'react';

import { appStore, valid_json } from './helper';
import { NotificationContext } from './Notification';
import { SimpleResizeEditor } from './Common';

function EditConfigFile(props: { handleClose }) {
  const { handleClose } = props;

  const [config, setConfig] = useState('');
  const [jsonError, setJsonError] = useState<string | null>(null);

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
    valid_json(config)
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
      for (const key in parsed) {
        appStore.set(key, parsed[key]);
      }
      showNotification('success', 'Success', 'Config file saved successfully');
      handleClose();
    } catch (e) {
      showNotification('error', 'Error', 'Failed to save config file: ' + e);
    }
  };
  return (
    <Stack spacing={1}>
      <SimpleResizeEditor
        value={config}
        onMount={(editor) => {
          editor.onDidChangeModelContent(() => {
            setConfig(editor.getValue());
          });
        }}
        heightName="edit-config-file"
        language="json"
        barLeft={
          jsonError ? (
            <Typography color="error"> {jsonError}</Typography>
          ) : undefined
        }
      ></SimpleResizeEditor>
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
