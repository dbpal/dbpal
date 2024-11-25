import { Editor } from '@monaco-editor/react';
import SplitPane from 'react-split-pane';
import { MultiQueryResp } from './QueryResp';

import './style/react-split-pane.css';
import {
  Button,
  CircularProgress,
  IconButton,
  Stack,
  Tooltip,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import PlayCircleIcon from '@mui/icons-material/PlayCircle';
import { useContext, useEffect, useState } from 'react';
import { editor } from 'monaco-editor';
import { EditorBar } from './common.tsx';
import { NotificationContext } from './Notification.tsx';
import types from './ApiStruct.ts';
import { QueryLog } from './QueryLog.tsx';

function SqlQuery(props: {
  language: string;
  queryOP: (text: string) => Promise<any[]>;
  loadQueryLog: () => Promise<types.QueryLogEvent[]>;
}) {
  let { language, queryOP, loadQueryLog } = props;
  const [resp, setResp] = useState([]);
  const [text, setText] = useState('');
  const [selectText, setSelectText] = useState('');
  const [pos, setPos] = useState<any>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [queryLogs, setQueryLogs] = useState([]);
  const handleEditorDidMount = (editor: editor.IStandaloneCodeEditor) => {
    editor.onDidChangeModelContent((_event) => {
      setText(editor.getValue());
    });
    editor.onDidChangeCursorPosition((_event) => {
      setPos(editor.getPosition());
    });
    editor.onDidChangeCursorSelection((_event) => {
      const selection = editor.getSelection();
      setSelectText(editor.getModel().getValueInRange(selection));
    });
  };

  const handleLoadQueryLog = () => {
    loadQueryLog()
      .then((logs) => {
        setQueryLogs(logs);
      })
      .catch((e) => {
        showNotification('error', 'fetch query logs failed', e);
      });
  };
  useEffect(() => {
    if (showQueryLog) {
      handleLoadQueryLog();
    }
  }, [showQueryLog]);

  const handleExecute = (text: string) => {
    setLoading(true);
    queryOP(text)
      .then((resp) => {
        setResp(resp);
        setLoading(false);
        if (showQueryLog) {
          handleLoadQueryLog();
        }
      })
      .catch((e) => {
        setLoading(false);
        setResp([]);
        showNotification('error', 'query failed', e);
      });
  };

  return (
    <SplitPane
      split='horizontal'
      defaultSize={500}
      minSize={50}
      style={{ overflow: 'auto' }}
    >
      <div
        style={{
          position: 'relative',
          width: '100%',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        // here is query log use positive to locate
        {showQueryLog && (
          <div
            style={{
              position: 'absolute',
              opacity: 1,
              right: 0,
              top: 0,
              display: showQueryLog ? 'block' : 'none',
              zIndex: 14,
              width: 700,
            }}
          >
            <QueryLog language={language} logs={queryLogs}></QueryLog>
          </div>
        )}
        <Stack direction='row' spacing={1} style={{ paddingLeft: '40px' }}>
          {loading && <CircularProgress />}
          <Tooltip title='execute'>
            <IconButton
              disabled={loading}
              onClick={() => {
                handleExecute(text);
              }}
              color='primary'
              aria-label='execute'
            >
              <PlayArrowIcon />
            </IconButton>
          </Tooltip>
          <Tooltip title='execute selected'>
            <IconButton
              disabled={loading}
              onClick={() => {
                handleExecute(selectText);
              }}
              color='primary'
              aria-label='execute'
            >
              <PlayCircleIcon />
            </IconButton>
          </Tooltip>
          <Button
            onClick={() => {
              setShowQueryLog((prev) => !prev);
            }}
          >
            {showQueryLog ? <CloseIcon></CloseIcon> : ''} query log
          </Button>
        </Stack>
        <div
          style={{
            flexGrow: 1,
            flexShrink: 1,
            minHeight: 0,
            height: '200px',
          }}
        >
          <Editor
            language={language}
            onMount={handleEditorDidMount}
            value=''
          ></Editor>
        </div>
        <EditorBar left={<></>} pos={pos}></EditorBar>
        {/* <div style={{ backgroundColor: 'gray', height: '100%' }}> 1111 </div> */}
      </div>

      <div>
        <MultiQueryResp data={resp}></MultiQueryResp>
      </div>
    </SplitPane>
  );
}

export { SqlQuery };
