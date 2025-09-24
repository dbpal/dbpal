import {
  Button,
  Chip,
  FormControlLabel,
  IconButton,
  Stack,
  Switch,
  Typography,
} from '@mui/material';
import { SimpleFlexEditor } from './CommonComp2';
import { useContext, useEffect, useRef, useState } from 'react';
import { appDataDir } from '@tauri-apps/api/path';
import { platform } from '@tauri-apps/plugin-os';
import { NotificationContext } from './Notification';
import { readFile, stat } from '@tauri-apps/plugin-fs';
import { editor, Position } from 'monaco-editor';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import {
  appStore,
  AppStoreKey,
  editorSetPositionAndReveal,
  UserSetting,
} from './helper';
import { invoke } from '@tauri-apps/api/core';

function getLogFilePath(): Promise<string> {
  return appDataDir().then((e) => {
    let name = 'dbpal.log';
    return platform() == 'windows' ? `${e}\\${name}` : `${e}/${name}`;
  });
}

function LogFile() {
  const [value, setValue] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [modifyDate, setModifyDate] = useState<string>('');
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [loggerConfig, setLoggerConfig] = useState<{
    level: string;
    max_size: number;
  }>(null);
  const [pinToBotton, setPinToBotton] = useState(false);
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
    appStore.get<UserSetting>(AppStoreKey.UserSetting).then((s) => {
      if (s) {
        setLoggerConfig(s.logger);
      }
    });
    let unlisten = appStore.onKeyChange<UserSetting>(
      AppStoreKey.UserSetting,
      (s) => {
        if (s) {
          setLoggerConfig(s.logger);
        }
      }
    );
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  useEffect(() => {
    handleReload();
  }, [modifyDate]);
  const handleTruncate = () => {
    invoke('truncate_log_file')
      .then(() => {
        showNotification('success', 'truncate log file OK');
      })
      .catch((e) => {
        showNotification('error', 'truncate log file error', e);
      });
  };

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

  useEffect(() => {
    if (pinToBotton) {
      editorSetPositionAndReveal(
        editor.current,
        new Position(value.split('\n').length + 1, 1)
      );
    }
  }, [value, pinToBotton]);
  return (
    <Stack spacing={1} style={{ height: '87vh' }}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <IconButton
          onClick={() => {
            editorSetPositionAndReveal(editor.current, new Position(1, 1));
          }}
          color="primary"
          disabled={pinToBotton}
        >
          <ArrowUpwardIcon></ArrowUpwardIcon>
        </IconButton>
        <IconButton
          onClick={() => {
            let lines = value.split('\n');
            editorSetPositionAndReveal(
              editor.current,
              new Position(lines.length + 1, 1)
            );
          }}
          color="primary"
          disabled={pinToBotton}
        >
          <ArrowDownwardIcon></ArrowDownwardIcon>
        </IconButton>
        <Button onClick={handleTruncate} variant="outlined">
          truncate
        </Button>
        <FormControlLabel
          label="pin to bottom"
          control=<Switch
            checked={pinToBotton}
            onChange={(e) => setPinToBotton(e.target.checked)}
          />
        ></FormControlLabel>

        {loggerConfig && (
          <>
            <Chip color="info" label={loggerConfig.level}></Chip>
            <Typography color="info">
              max size:{loggerConfig.max_size}MB
            </Typography>
          </>
        )}
        <span> loaded at:{modifyDate} </span>
      </Stack>

      <SimpleFlexEditor
        value={value}
        handleEditor={(ed) => {
          editor.current = ed;
        }}
        readOnly
        language="log"
      ></SimpleFlexEditor>
    </Stack>
  );
}

export { LogFile };
