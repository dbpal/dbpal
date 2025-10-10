'use strict';
import {
  Button,
  FormControlLabel,
  IconButton,
  Stack,
  Switch,
  Tooltip,
  Typography,
} from '@mui/material';
import { SimpleFlexEditor } from './CommonComp2';
import { useContext, useEffect, useState } from 'react';
import { editor, Position } from 'monaco-editor';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import { editorSetPositionAndReveal, LoggerLevel } from './helper';
import { LogLevelSelection } from './CommonComp';
import { NotificationContext } from './Notification';
import InfoIcon from '@mui/icons-material/Info';
import { Channel, invoke } from '@tauri-apps/api/core';
function LogTip({ width }) {
  return (
    <Stack style={{ width }} spacing={1}>
      <Typography variant="h6">
        The rust backend can generate various logs,it is useful for debug a lot
        of problem.
      </Typography>

      <Typography>
        Dbpal will log sql been execute via `log::info("sql:" ,sql)`,so you can
        search for them and dependencies can generate various logs maybe very
        useful.
      </Typography>
      <Typography color="warning">
        dbpal will not trying to save logs to local file because maybe be
        danger.
        <br />
        When this window is closed,the log will be lost.
      </Typography>
    </Stack>
  );
}

function LogFile() {
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pinToBotton, setPinToBotton] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [loggerLevel, setLoggerLevel] = useState(LoggerLevel.level);
  useEffect(() => {
    if (!editor) {
      return;
    }
    const onEvent = new Channel<string>();
    onEvent.onmessage = (message) => {
      const model = editor.getModel()!; // 或 monaco.editor.getModels()[0]
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
          text: message,
        },
      ]);
      if (pinToBotton) {
        editorSetPositionAndReveal(editor, new Position(lastLine, 1));
      }
    };
    invoke('log_channel', {
      onEvent,
    });
  }, [pinToBotton, editor]);

  return (
    <Stack spacing={1} style={{ height: '100%' }}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <IconButton
          onClick={() => {
            editorSetPositionAndReveal(editor, new Position(1, 1));
          }}
          color="primary"
          disabled={pinToBotton}
        >
          <ArrowUpwardIcon></ArrowUpwardIcon>
        </IconButton>
        <IconButton
          onClick={() => {
            const model = editor.getModel()!; // 或 monaco.editor.getModels()[0]
            const lastLine = model.getLineCount();
            editorSetPositionAndReveal(editor, new Position(lastLine, 1));
          }}
          color="primary"
          disabled={pinToBotton}
        >
          <ArrowDownwardIcon></ArrowDownwardIcon>
        </IconButton>
        <Button
          variant="outlined"
          onClick={() => {
            editor.getModel().setValue('');
            showNotification('success', 'log truncated');
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
          level={loggerLevel}
          handleChange={async (level) => {
            try {
              await LoggerLevel.set(level);
              setLoggerLevel(level);
            } catch (e) {
              showNotification('error', 'save log level failed', e);
              return;
            }
          }}
        ></LogLevelSelection>

        <Tooltip
          title={<LogTip width="40vw"></LogTip>}
          slotProps={{
            tooltip: {
              style: { maxWidth: '40vw' },
            },
          }}
        >
          <span>
            <InfoIcon> </InfoIcon>
          </span>
        </Tooltip>
      </Stack>

      <SimpleFlexEditor
        value={''}
        handleEditor={(ed) => {
          setEditor(ed);
        }}
        readOnly
        language="log"
      ></SimpleFlexEditor>
    </Stack>
  );
}

export { LogFile };
