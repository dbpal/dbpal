import React, {
  useState,
  ReactNode,
  createContext,
  ReactElement,
  useMemo,
} from "react";
import { Alert, AlertTitle, Stack, Typography } from "@mui/material";
import { AlertColor } from "@mui/material/Alert";
import { message } from "@tauri-apps/plugin-dialog";
const msgTimeout = 10000;
interface NotificationContextProps {
  showNotification: (
    severity: AlertColor,
    title: string,
    message?: string | Error | ReactElement,
  ) => void;
}

export const NotificationContext = createContext<NotificationContextProps>({
  showNotification: (...args) => {
    message("should not call this function is is bug", {
      kind: "error",
    });
    console.error("message args:", args);
  },
});

function NotificationProvider(props: { children: ReactNode }) {
  const { children } = props;
  const [notifications, setNotifications] = useState<
    {
      severity: AlertColor;
      title: string;
      message: ReactElement | string | null | undefined;
    }[]
  >([]);

  const showNotification = useMemo(() => {
    const showNotification = (severity: AlertColor, title: string, err) => {
      // const iscancelled = isCancelledError(err);
      if (typeof err == "object" && "reason" in err) {
        console.error("detail:", err.detail);
        // this a error from the backend
        // we only show `reason` the short message
        err = err["reason"];
      }
      if (!React.isValidElement(err) && err) {
        err = String(err);
      }
      setNotifications((e) => [...e, { severity, title, message: err }]);
      setTimeout(() => {
        setNotifications((e) => e.slice(1));
      }, msgTimeout);
    };
    return { showNotification };
  }, []);

  return (
    <NotificationContext.Provider value={showNotification}>
      {children}
      {open && (
        <Stack
          style={{
            overflow: "auto",
            position: "fixed",
            right: 0,
            top: 0,
            zIndex: 14,
          }}
        >
          {notifications.map((notice, index) => (
            <Alert
              key={index}
              severity={notice.severity}
              sx={{ maxWidth: "45vw", minWidth: "15vw" }}
              onClose={() => {
                setNotifications((e) => e.filter((_, i) => i !== index));
              }}
            >
              {/* 设置更宽的宽度 */}
              {notice.title && (
                <AlertTitle color={notice.severity}>{notice.title}</AlertTitle>
              )}
              <Stack direction="row" style={{ alignItems: "flex-start" }}>
                {React.isValidElement(notice.message) ? (
                  notice.message
                ) : (
                  <Typography color={notice.severity}>{notice.message}</Typography>
                )}
              </Stack>
            </Alert>
          ))}
        </Stack>
      )}
    </NotificationContext.Provider>
  );
}

export { NotificationProvider };
