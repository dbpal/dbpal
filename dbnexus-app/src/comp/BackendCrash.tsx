import { Button, Paper, Stack } from '@mui/material';
import { openUrl } from '@tauri-apps/plugin-opener';
import { handleCopyToClipboard } from './helper';
import { useContext } from 'react';
import { NotificationContext } from './Notification';

function BackendCrash(props: { msg: string }) {
  let { msg } = props;

  const { showNotification } = useContext(NotificationContext);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          onClick={() => {
            openUrl('https://github.com/dbpal/docs/issues');
          }}
          color="error"
          variant="contained"
        >
          report
        </Button>
        <Button
          variant="contained"
          onClick={() => {
            handleCopyToClipboard(msg, showNotification);
          }}
        >
          copy error msg
        </Button>
      </Stack>

      <Paper elevation={8}>
        <pre style={{ color: 'red', fontWeight: 'bold', overflow: 'scorll' }}>
          {msg}
        </pre>
      </Paper>
    </Stack>
  );
}

export { BackendCrash };
