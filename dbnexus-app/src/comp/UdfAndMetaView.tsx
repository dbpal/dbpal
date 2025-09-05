import { useContext, useEffect, useRef, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, Stack, Tooltip, Typography } from '@mui/material';
import { DefAndMetas, SimpleFlexEditor } from './Common';
import { MetaValuesView, PossibleParseError } from './CommonComp';
import { ErrAndLoc, getEditorText, saveAndSetSize } from './helper';
import { MultiQueryResp } from './QueryResp';
import { editor } from 'monaco-editor';

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  executeOP: (sql: string) => Promise<any>;
  getParseErr: (sql: string) => Promise<ErrAndLoc | null>;
  language: string;
  showReload?: boolean;
  heightName?: string;
}) {
  let { reloadOP, language, showReload, heightName, executeOP, getParseErr } =
    props;
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
  const [parseErr, setParseErr] = useState<ErrAndLoc>(null);
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
        <Tooltip title="execute the sql">
          <Button
            loading={executeLoading}
            color="primary"
            style={{
              minWidth: '3rem',
            }}
            onClick={() => {
              handleExecute();
            }}
            variant="contained"
          >
            execute
          </Button>
        </Tooltip>
      </Stack>
      {detail.def && (
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
          <SimpleFlexEditor
            language={language}
            value={detail.def}
            editorRef={editor}
            handleValue={(sql) => {
              getParseErr(sql).then((e) => {
                setParseErr(e);
              });
            }}
            barLeft=<Stack direction="row" flex={1}>
              {parseErr && (
                <PossibleParseError
                  parseErr={parseErr}
                  editor={editor.current}
                ></PossibleParseError>
              )}
            </Stack>
          ></SimpleFlexEditor>
        </div>
      )}
      <MultiQueryResp data={queryResp} />
      {detail.metas && detail.metas.length > 0 && (
        <>
          <Typography variant={titleVariant}> Some Metas </Typography>
          <MetaValuesView values={detail.metas}></MetaValuesView>
        </>
      )}
    </Stack>
  );
}

export { DefAndMetasView };
