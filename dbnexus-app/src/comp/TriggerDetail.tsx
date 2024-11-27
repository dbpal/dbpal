import { ReactNode, useContext, useState } from 'react';
import { types } from './ApiStruct.ts';
import { DefAndMetasView } from './UdfAndMetaView.tsx';
import { Button, CircularProgress, Stack, Typography } from '@mui/material';
import { NotificationContext } from './Notification.tsx';
import { DefAndMetas } from './common.tsx';

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
  return [
    { name: 'timming', value: x.timing, desc: '' },
    { name: 'event', value: x.event, desc: '' },
    {
      name: 'enable',
      value: getEnableStr(),
      desc: getEnableDesc(),
    },
  ];
}

function triggerIsEnable(enable: types.TriggerEnable): boolean {
  if (enable.kind == 'postgres') {
    return enable.enable.toUpperCase() != 'D';
  }
  if (enable.kind == 'bool') {
    return enable.enable;
  }
}

function TriggerDetail(props: {
  reloadOP: () => Promise<types.TriggerDetail>;
  dropOP: () => Promise<void>;
  disableOP: () => Promise<void>;
  enableOP: () => Promise<void>;
  language: string;
}) {
  const { reloadOP, dropOP, language, disableOP, enableOP } = props;
  // this loading status only used in enable or disable trigger.

  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleLoading = (x: () => Promise<void>, errMsg: string) => {
    setLoading(true);
    x()
      .then(() => {
        setLoading(false);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', errMsg, e);
      });
  };

  const titleVariant = 'h3';

  const extraEnabless = (
    detail: types.TriggerDetail,
    handleReload: () => void
  ): ReactNode => {
    return triggerIsEnable(detail.enable) ? (
      <Stack>
        <Typography variant={titleVariant}> Disable Trigger </Typography>
        <Stack direction='row'>
          <Button
            variant='contained'
            onClick={() => {
              handleLoading(async () => {
                try {
                  await disableOP();
                } catch (e) {
                  throw e;
                }
                handleReload();
              }, 'reload failed');
            }}
          >
            Disable
          </Button>
          {loading && <CircularProgress />}
        </Stack>
      </Stack>
    ) : (
      <Stack>
        <Typography variant={titleVariant}> Enable Trigger </Typography>
        <Stack direction='row'>
          <Button
            variant='contained'
            onClick={() => {
              handleLoading(async () => {
                try {
                  await enableOP();
                } catch (e) {
                  throw e;
                }
                handleReload();
              }, 'reload failed');
            }}
          >
            Disable
          </Button>
          {loading && <CircularProgress />}
        </Stack>
      </Stack>
    );
  };
  return (
    <DefAndMetasView
      dropText='Drop trigger'
      afterDropMsg='The trigger is droped,please close this tab'
      language={language}
      dropOP={dropOP}
      reloadOP={async () => {
        return reloadOP().then((detail) => {
          return {
            def: detail.sql,
            metas: [...triggerDetailsToMetas(detail), ...detail.metas],
            detail,
          };
        });
      }}
      extra={(x: DefAndMetas, reload) => {
        //@ts-ignore
        return extraEnabless(x.detail, reload);
      }}
      titleVariant={titleVariant}
    ></DefAndMetasView>
  );
}

export { TriggerDetail };
