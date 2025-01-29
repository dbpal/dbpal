import React, { useState, ReactNode, createContext } from 'react';
import { Snackbar, Alert, AlertTitle, Slide } from '@mui/material';
import { AlertColor } from '@mui/material/Alert';
import { msgTimeout } from './Common';

interface NotificationContextProps {
  showNotification: (
    severity: AlertColor,
    title: string,
    message?: string | Error
  ) => void;
}

export const NotificationContext = createContext<NotificationContextProps>({
  showNotification: () => {},
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
    message: '',
  });

  const showNotification = (
    severity: AlertColor,
    title: string,
    message: string | Error | { reason: string }
  ) => {
    if (typeof message == 'object' && 'reason' in message) {
      // TODO show message
      message = message['reason'];
    }
    if (typeof message == 'object' && message instanceof Error) {
      // TODO show message
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

  return (
    <NotificationContext.Provider value={{ showNotification }}>
      {children}
      {open && (
        <Snackbar
          open={open}
          autoHideDuration={msgTimeout}
          onClose={handleClose}
          anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
          TransitionComponent={transition}
          sx={{ mb: 8 }} // 调整位置稍微高一些
        >
          <Alert severity={notification.severity} sx={{ width: '400px' }}>
            {/* 设置更宽的宽度 */}
            {notification.title && (
              <AlertTitle>{notification.title}</AlertTitle>
            )}
            {notification.message}
          </Alert>
        </Snackbar>
      )}
    </NotificationContext.Provider>
  );
};
