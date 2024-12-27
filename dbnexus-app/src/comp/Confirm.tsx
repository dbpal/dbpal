import { ask } from '@tauri-apps/plugin-dialog';

interface ConfirmOptions {
  title: string;
  content?: string;
  confirmText?: string;
  cancelText?: string;
}

export async function secondConfirm({
  title,
  content,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
}: ConfirmOptions): Promise<boolean> {
  return ask(title, {
    title: content,
    kind: 'error',
    okLabel: confirmText,
    cancelLabel: cancelText,
  });
}
