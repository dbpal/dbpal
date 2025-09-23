import { Button, Paper, Stack, useTheme } from '@mui/material';
import { openUrl } from '@tauri-apps/plugin-opener';
import { handleCopyToClipboard } from './helper';
import { useContext } from 'react';
import { NotificationContext } from './Notification';

function BackendCrash(props: { msg: string }) {
  let { msg } = props;
  let theme = useTheme();

  const { showNotification } = useContext(NotificationContext);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          onClick={() => {
            openUrl('https://github.com/dbpal/docs/issues');
          }}
          color="info"
        >
          report
        </Button>
        <Button
          variant="outlined"
          onClick={() => {
            handleCopyToClipboard(msg, showNotification);
          }}
        >
          copy error msg
        </Button>
      </Stack>

      <div style={{ display: 'grid' }}>
        <Paper
          elevation={8}
          style={{
            overflow: 'scroll',
            maxWidth: '99%',
            maxHeight: '80vh',
            padding: '2px 8px',
          }}
        >
          <pre style={{ color: theme.palette.error.main }}>{msg}</pre>
        </Paper>
      </div>
    </Stack>
  );
}

export { BackendCrash };
