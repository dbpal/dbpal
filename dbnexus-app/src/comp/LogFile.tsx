import { Button, Stack } from '@mui/material';
import { SimpleFlexEditor } from './CommonComp2';
import { useContext, useEffect, useRef, useState } from 'react';
import { appDataDir } from '@tauri-apps/api/path';
import { platform } from '@tauri-apps/plugin-os';
import { NotificationContext } from './Notification';
import { readFile, stat, truncate, writeFile } from '@tauri-apps/plugin-fs';
import { editor, Position } from 'monaco-editor';
import { editorSetPositionAndReveal } from './helper';

function getLogFilePath(): Promise<string> {
  return appDataDir().then((e) => {
    let name = 'dbpal.log';
    return platform() == 'windows' ? `e\\${name}` : `${e}/${name}`;
  });
}

function LogFile() {
  const [value, setValue] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [modifyDate, setModifyDate] = useState<string>('');
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const handleReload = async () => {
    let path;
    try {
      path = await getLogFilePath();
    } catch (e) {
      showNotification('error', 'get log file path error', e);
      return;
    }
    try {
      let info = await stat(path);
      if (info.mtime) {
        setModifyDate(info.mtime.toLocaleString());
      }
      const content = await readFile(path);
      const str = new TextDecoder().decode(content);
      setValue(str);
    } catch (e) {
      showNotification('error', 'read log file error', e);
      return;
    }
  };
  useEffect(() => {
    handleReload();
  }, [modifyDate]);

  useEffect(() => {
    const int = setInterval(() => {
      (async () => {
        let info = await stat(await getLogFilePath());
        if (info.mtime) {
          setModifyDate(info.mtime.toLocaleString());
        }
      })();
    }, 1000);
    return () => {
      clearInterval(int);
    };
  }, []);
  return (
    <Stack spacing={1} style={{ height: '87vh' }}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          onClick={() => {
            let lines = value.split('\n');
            editorSetPositionAndReveal(
              editor.current,
              new Position(lines.length + 1, 1)
            );
          }}
          variant="outlined"
        >
          go to bottom
        </Button>
        <Button
          onClick={() => {
            editorSetPositionAndReveal(editor.current, new Position(1, 1));
          }}
          variant="outlined"
        >
          go to top
        </Button>
        <span> modify at:{modifyDate} </span>
      </Stack>

      <SimpleFlexEditor
        value={value}
        handleEditor={(ed) => {
          editor.current = ed;
        }}
        language="log"
      ></SimpleFlexEditor>
    </Stack>
  );
}

export { LogFile };
