import React from "react";
import { Alert, AlertTitle, Stack, Typography } from "@mui/material";
import { useNotification } from "./store";

function Notifications() {
  const { notifications, closeMsg } = useNotification();

  return (
    notifications.length > 0 && (
      <Stack
        style={{
          overflow: "auto",
          position: "fixed",
          right: 0,
          top: 0,
          zIndex: 11114,
        }}>
        {notifications.map((notice, index) => (
          <Alert
            key={index}
            severity={notice.severity}
            sx={{ maxWidth: "45vw", minWidth: "15vw" }}
            onClose={() => {
              closeMsg(index);
            }}>
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
    )
  );
}

export { Notifications };
