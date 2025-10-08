'use strict';
import React, {
  useState,
  ReactNode,
  createContext,
  useEffect,
  ReactElement,
} from 'react';
import {
  Snackbar,
  Alert,
  AlertTitle,
  Slide,
  Stack,
  Button,
  Typography,
} from '@mui/material';
import { AlertColor } from '@mui/material/Alert';
import { msgTimeout } from './Common';
import { handleCopyToClipboard } from './helper';

interface NotificationContextProps {
  showNotification: (
    severity: AlertColor,
    title: string,
    message?: string | Error | ReactElement
  ) => void;
}

export const NotificationContext = createContext<NotificationContextProps>({
  showNotification: (...args) => {
    console.error('should not call this function is is bug ,args :', args);
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
    message: '' as any,
  });

  const showNotification = (
    severity: AlertColor,
    title: string,
    message: string | Error | { reason: string } | ReactElement
  ) => {
    if (typeof message == 'object' && 'reason' in message) {
      console.log('detail:', message);
      // this a error from the backend
      // we only show `reason` the short message
      message = message['reason'];
    }
    if (typeof message == 'object' && message instanceof Error) {
      // error  to string
      message = message.toString();
    }
    setNotification({ severity, title, message });
    setOpen(true);
  };

  const handleClose = () => {
    setOpen(false);
  };

  // 定义动画效果
  const transition = (props: any) => {
    return <Slide {...props} direction="left" />;
  };
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    setCopied(false);
  }, [notification]);
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
          slots={{ transition: copied ? undefined : transition }}
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
              {notification.message &&
                typeof notification.message == 'string' && (
                  <Button
                    onClick={() => {
                      handleCopyToClipboard(
                        notification.message,
                        undefined,
                        () => {
                          setCopied(true);
                        }
                      );
                    }}
                    variant="text"
                    color={copied ? 'success' : 'primary'}
                  >
                    {copied ? 'copied' : 'copy'}
                  </Button>
                )}
            </Stack>
          </Alert>
        </Snackbar>
      )}
    </NotificationContext.Provider>
  );
};
