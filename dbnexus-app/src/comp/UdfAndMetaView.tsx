import { useContext, useEffect, useRef, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, Stack, Tooltip, Typography } from '@mui/material';
import { DefAndMetas, setAutoCompletionName } from './Common';
import { MetaValueView, PossibleParseError } from './CommonComp';
import {
  DataSourceKind,
  ErrAndLoc,
  getEditorText,
  makeExecuteSql,
  makeGetParserErr,
  saveAndSetSize,
  SettingContext,
  TabActiveContext,
  WindowFocusContext,
} from './helper';
import { MultiQueryResp } from './QueryResp';
import { editor } from 'monaco-editor';
import { SimpleFlexEditor } from './CommonComp2';
import types from './ApiStruct';

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  uuid: string;
  kind: DataSourceKind;
  showReload?: boolean;
  heightName?: string;
}) {
  let { reloadOP, kind, showReload, heightName, uuid } = props;
  const setting = useContext(SettingContext);
  const executeOP = makeExecuteSql(uuid, setting.saveQueryLog);
  const getParseErr = makeGetParserErr(uuid);
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
  const handleReload = (showOk) => {
    setLoading(true);
    reloadOP()
      .then((detail) => {
        setDetail(detail);
        if (showOk) {
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
  const active = useContext(TabActiveContext);
  const focus = useContext(WindowFocusContext);
  useEffect(() => {
    if (active && focus) {
      setAutoCompletionName(uuid);
    }
  }, [active, focus]);
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
            color="success"
            style={{
              minWidth: '3rem',
            }}
            onClick={() => {
              handleExecute();
            }}
          >
            execute!
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
            language={kind}
            value={detail.def}
            handleEditor={(ed) => {
              editor.current = ed;
            }}
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
          <MetaValueView values={detail.metas}></MetaValueView>
        </>
      )}
    </Stack>
  );
}

function triggerDetailsToMetas(x: types.TriggerDetail): types.MetaValue[] {
  const getEnableStr = (): string => {
    return x.enable.enable + '';
  };

  const getEnableDesc = (): string => {
    if (x.enable.kind == 'postgres') {
      return `Controls in which session_replication_role modes the trigger fires. O = trigger fires in
      “origin” and “local” modes, D = trigger is disabled, R = trigger fires in “replica” mode, A
      = trigger fires always.`;
    }
    return '';
  };
  const ret: types.MetaValue[] = [
    // todo timing can be object
    { name: 'timing', value: x.timing as string, desc: '' },
    { name: 'event', value: x.event, desc: '' },
  ];
  if (x.enable) {
    ret.push({
      name: 'enable',
      value: getEnableStr(),
      desc: getEnableDesc(),
    });
  }
  return ret;
}

function TriggerDetail(props: {
  reloadOP: () => Promise<types.TriggerDetail>;
  uuid: string;
  kind: DataSourceKind;
}) {
  const { reloadOP, uuid, kind } = props;

  return (
    <DefAndMetasView
      heightName="trigger-detail"
      kind={kind}
      reloadOP={async () => {
        return reloadOP().then((detail) => {
          return {
            def: detail.sql,
            metas: [...triggerDetailsToMetas(detail), ...detail.metas],
            detail,
          };
        });
      }}
      uuid={uuid}
    ></DefAndMetasView>
  );
}
export { DefAndMetasView, TriggerDetail };
