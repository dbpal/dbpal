import React, { useState, ReactNode, createContext } from 'react';
import { Snackbar, Alert, AlertTitle, Slide } from '@mui/material';
import { AlertColor } from '@mui/material/Alert';
import { WidthFull } from '@mui/icons-material';

interface NotificationContextProps {
  showNotification: (
    severity: AlertColor,
    title: string,
    message: string | Error
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
    message: string
  ) => {
    // 首先关闭 Snackbar，确保每次可以重新打开
    setOpen(false);
    setNotification({ severity, title, message });
    setTimeout(() => setOpen(true), 0); // 延时打开，确保状态变化
  };

  const handleClose = () => {
    setOpen(false);
  };

  // 定义动画效果
  const transition = (props: any) => {
    return <Slide {...props} direction='left' />;
  };

  return (
    <NotificationContext.Provider value={{ showNotification }}>
      {children}
      <Snackbar
        open={open}
        autoHideDuration={5000} // 显示 5 秒
        onClose={handleClose}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
        TransitionComponent={transition}
        sx={{ mb: 8 }} // 调整位置稍微高一些
      >
        <Alert severity={notification.severity} sx={{ width: '300px' }}>
          {' '}
          {/* 设置更宽的宽度 */}
          {notification.title && <AlertTitle>{notification.title}</AlertTitle>}
          {notification.message}
        </Alert>
      </Snackbar>
    </NotificationContext.Provider>
  );
};
