import { useContext, useEffect, useRef, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, IconButton, Stack, Tooltip, Typography } from '@mui/material';
import { DefAndMetas, SimpleFlexSizeEditor } from './Common';
import { MetaValuesView } from './CommonComp';
import { getEditorText, saveAndSetSize } from './helper';
import { FaExclamation } from 'react-icons/fa';
import { MultiQueryResp } from './QueryResp';
import { editor } from 'monaco-editor';
function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  executeOP: (sql: string) => Promise<any>;
  language: string;
  showReload?: boolean;
  heightName?: string;
}) {
  let { reloadOP, language, showReload, heightName, executeOP } = props;
  const titleVariant = 'h6';
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas>({
    def: '',
    metas: [],
  });
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [queryResp, setQueryResp] = useState<any[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [executeLoading, setExecuteLoading] = useState(false);
  const handleReload = (showok) => {
    setLoading(true);
    reloadOP()
      .then((detail) => {
        setDetail(detail);
        if (showok) {
          showNotification('success', 'reload ok');
        }
      })
      .catch((e) => {
        showNotification('error', 'fetch detail failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const handleExecute = () => {
    let sql = getEditorText(editor.current, showNotification);
    setExecuteLoading(true);
    executeOP(sql)
      .then((e) => {
        setQueryResp([e]);
      })
      .catch((e) => {
        showNotification('error', 'execute failed', e);
      })
      .finally(() => {
        setExecuteLoading(false);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const [dom, setDom] = useState(null);
  useEffect(() => {
    if (dom) {
      let c = saveAndSetSize(
        dom,
        heightName ? heightName : 'udf-metas-view',
        'height'
      );
      return c;
    }
  }, [dom]);
  return (
    <Stack spacing={1} style={{ minHeight: '18vh' }}>
      <Stack direction="row" spacing={1}>
        {showReload && (
          <Button
            loading={loading}
            variant="contained"
            color="primary"
            onClick={() => {
              handleReload(true);
            }}
          >
            reload
          </Button>
        )}
        <Tooltip title="execute sql">
          <IconButton loading={executeLoading}>
            <FaExclamation
              onClick={() => {
                handleExecute();
              }}
            ></FaExclamation>
          </IconButton>
        </Tooltip>
      </Stack>
      <div
        ref={setDom}
        style={{
          display: 'flex',
          flexDirection: 'column',
          overflow: 'auto',
          minHeight: '15vh',
          resize: 'vertical',
          height: '18vh',
        }}
      >
        <SimpleFlexSizeEditor
          language={language}
          value={detail.def}
          editorRef={editor}
        ></SimpleFlexSizeEditor>
      </div>
      {detail.metas.length > 0 && (
        <>
          <Typography variant={titleVariant}> Some Metas </Typography>
          <MetaValuesView values={detail.metas}></MetaValuesView>
        </>
      )}
      <MultiQueryResp data={queryResp} />
    </Stack>
  );
}

export { DefAndMetasView };
