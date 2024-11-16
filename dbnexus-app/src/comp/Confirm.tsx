import {
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Button,
} from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  content: string;
  confirmText: string;
  cancelText: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  title,
  content,
  confirmText,
  cancelText,
  onConfirm,
  onCancel,
}) => {
  return (
    <Dialog open={open} onClose={onCancel} sx={{ pb: 16 }}>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText>{content}</DialogContentText>
      </DialogContent>
      <DialogActions sx={{ p: 3 }}>
        <Button onClick={onCancel} color='primary' variant='outlined'>
          {cancelText}
        </Button>
        <Button onClick={onConfirm} color='error' variant='contained'>
          {confirmText}
        </Button>
      </DialogActions>
    </Dialog>
  );
};

interface ConfirmOptions {
  title: string;
  content: string;
  confirmText?: string;
  cancelText?: string;
}

export async function secondConfirm({
  title,
  content,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
}: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    const div = document.createElement('div');
    document.body.appendChild(div);

    const root = ReactDOM.createRoot(div);

    const handleConfirm = () => {
      cleanup();
      resolve(true);
    };

    const handleCancel = () => {
      cleanup();
      resolve(false);
    };

    const cleanup = () => {
      root.unmount();
      div.remove();
    };

    root.render(
      <ConfirmDialog
        open={true}
        title={title}
        content={content}
        confirmText={confirmText}
        cancelText={cancelText}
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    );
  });
}
