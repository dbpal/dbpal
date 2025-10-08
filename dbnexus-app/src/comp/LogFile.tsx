'use strict';
import {
  Button,
  FormControlLabel,
  IconButton,
  Stack,
  Switch,
} from '@mui/material';
import { SimpleFlexEditor } from './CommonComp2';
import { useContext, useEffect, useRef, useState } from 'react';
import { editor, Position } from 'monaco-editor';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import {
  editorSetPositionAndReveal,
  SettingContext,
  UserSetting,
} from './helper';
import { listen } from '@tauri-apps/api/event';
import { LogLevelSelection } from './CommonComp';
import { NotificationContext } from './Notification';

function LogFile() {
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const setting = useContext(SettingContext);
  const [pinToBotton, setPinToBotton] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    const unlisten = listen<string>('log_stream', (e) => {
      const model = editor.current.getModel()!; // 或 monaco.editor.getModels()[0]
      const lastLine = model.getLineCount();
      const lastCol = model.getLineMaxColumn(lastLine);
      model.applyEdits([
        {
          range: {
            startLineNumber: lastLine,
            startColumn: lastCol,
            endLineNumber: lastLine,
            endColumn: lastCol,
          },
          text: e.payload,
        },
      ]);
      if (pinToBotton) {
        console.log('ping to botton');
        editorSetPositionAndReveal(editor.current, new Position(lastLine, 1));
      }
    });
    return () => {
      unlisten.then((unlisten) => {
        unlisten();
      });
    };
  }, [pinToBotton]);

  return (
    <Stack spacing={1} style={{ height: '100%' }}>
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
            const model = editor.current.getModel()!; // 或 monaco.editor.getModels()[0]
            const lastLine = model.getLineCount();
            editorSetPositionAndReveal(
              editor.current,
              new Position(lastLine, 1)
            );
          }}
          color="primary"
          disabled={pinToBotton}
        >
          <ArrowDownwardIcon></ArrowDownwardIcon>
        </IconButton>
        <Button
          variant="outlined"
          onClick={() => {
            editor.current.getModel().setValue('');
          }}
        >
          truncate
        </Button>
        <FormControlLabel
          label="pin to bottom"
          control=<Switch
            checked={pinToBotton}
            onChange={(e) => setPinToBotton(e.target.checked)}
          />
        ></FormControlLabel>
        <LogLevelSelection
          level={setting.loggerLevel}
          handleChange={(level) => {
            try {
              UserSetting.set({ loggerLevel: level } as UserSetting);
            } catch (e) {
              showNotification('error', 'save log level failed', e);
              return;
            }
          }}
        ></LogLevelSelection>
      </Stack>

      <SimpleFlexEditor
        value={''}
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
