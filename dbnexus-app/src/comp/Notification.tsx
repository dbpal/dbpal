'use strict';
import React, { useState, ReactNode, createContext, ReactElement } from 'react';
import {
  Snackbar,
  Alert,
  AlertTitle,
  Slide,
  Stack,
  Typography,
} from '@mui/material';
import { AlertColor } from '@mui/material/Alert';
import { handleCopyToClipboard } from './helper';
import { message, MessageDialogResult } from '@tauri-apps/plugin-dialog';

const msgTimeout = 10000;

interface NotificationContextProps {
  showNotification: (
    severity: AlertColor,
    title: string,
    message?: string | Error | ReactElement
  ) => void;
}

export const NotificationContext = createContext<NotificationContextProps>({
  showNotification: (...args) => {
    message('should not call this function is is bug', {
      kind: 'error',
    });
    console.log('message args:', args);
  },
});

interface NotificationProviderProps {
  children: ReactNode;
}

export const NotificationProvider: React.FC<NotificationProviderProps> = ({
  children,
}) => {
  const [open, setOpen] = useState(false);
  const [notification, setNotification] = useState({
    severity: 'info' as AlertColor,
    title: '',
    message: '' as ReactElement | string | null | undefined,
  });

  const showNotification = (
    severity: AlertColor,
    title: string,
    msg: any | ReactElement
  ) => {
    if (typeof msg == 'object' && 'reason' in msg) {
      console.log('detail:', msg);
      // this a error from the backend
      // we only show `reason` the short message
      msg = msg['reason'];
    }
    if (severity == 'error' && !React.isValidElement(msg)) {
      msg = msg ? String(msg) : '';
      message(msg, {
        kind: severity,
        title,
        buttons: {
          ok: msg ? 'copy error' : 'ok',
          cancel: 'dismiss',
        },
      }).then((e: MessageDialogResult) => {
        if (e == 'copy error') {
          handleCopyToClipboard(msg);
        }
      });
      return;
    }
    if (!React.isValidElement(msg) && msg) {
      msg = String(msg);
    }
    setNotification({ severity, title, message: msg });
    setOpen(true);
  };

  const handleClose = () => {
    setOpen(false);
  };

  // 定义动画效果
  const transition = (props: any) => {
    return <Slide {...props} direction="left" />;
  };

  return (
    <NotificationContext.Provider value={{ showNotification }}>
      {children}
      {open && (
        <Snackbar
          open={open}
          autoHideDuration={msgTimeout}
          onClose={handleClose}
          anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
          sx={{ mb: 8 }} // 调整位置稍微高一些
          slots={{ transition }}
        >
          <Alert
            severity={notification.severity}
            sx={{ maxWidth: '60vw', minWidth: '15vw' }}
          >
            {/* 设置更宽的宽度 */}
            {notification.title && (
              <AlertTitle color={notification.severity}>
                {notification.title}
              </AlertTitle>
            )}
            <Stack
              direction="row"
              spacing={1}
              style={{ alignItems: 'flex-start' }}
            >
              {React.isValidElement(notification.message) ? (
                notification.message
              ) : (
                <Typography color={notification.severity}>
                  {notification.message}
                </Typography>
              )}
            </Stack>
          </Alert>
        </Snackbar>
      )}
    </NotificationContext.Provider>
  );
};
