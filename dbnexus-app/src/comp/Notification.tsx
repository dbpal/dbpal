import React, { useState, ReactNode, createContext, ReactElement } from "react";
import { Snackbar, Alert, AlertTitle, Slide, Stack, Typography } from "@mui/material";
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

  const [open, setOpen] = useState(false);
  const [notification, setNotification] = useState({
    severity: "info" as AlertColor,
    title: "",
    message: "" as ReactElement | string | null | undefined,
  });

  const showNotification = (severity: AlertColor, title: string, err) => {
    if (!severity) {
      setOpen(false);
      return;
    }
    // const iscancelled = isCancelledError(err);
    if (typeof err == "object" && "reason" in err) {
      console.error("detail:", err);
      // this a error from the backend
      // we only show `reason` the short message
      err = err["reason"];
    }
    // if (severity == "error" && !React.isValidElement(err) && !iscancelled) {
    //   err = err ? String(err) : "";
    //   message(err, {
    //     kind: severity,
    //     title,
    //     buttons: {
    //       ok: err ? "copy error" : "ok",
    //       cancel: "dismiss",
    //     },
    //   }).then((e: MessageDialogResult) => {
    //     if (e == "copy error") {
    //       handleCopyToClipboard(err);
    //     }
    //   });
    //   return;
    // }
    if (!React.isValidElement(err) && err) {
      err = String(err);
    }
    setOpen(false);
    setTimeout(() => {
      setNotification({ severity, title, message: err });
      setOpen(true);
    }, 100);
  };
  const handleClose = (_, reason) => {
    if (reason === "clickaway") {
      return;
    }
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
          anchorOrigin={{ vertical: "top", horizontal: "right" }}
          slots={{ transition }}
        >
          <Alert
            severity={notification.severity}
            sx={{ maxWidth: "45vw", minWidth: "15vw" }}
            onClose={() => {
              setOpen(false);
            }}
          >
            {/* 设置更宽的宽度 */}
            {notification.title && (
              <AlertTitle color={notification.severity}>
                {notification.title}
              </AlertTitle>
            )}
            <Stack direction="row" style={{ alignItems: "flex-start" }}>
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
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
//@ts-ignore
function isCancelledError(e: { cancelled?: boolean }) {
  return typeof e == "object" && e.cancelled;
}
export { NotificationProvider };
