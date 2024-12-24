import { useContext, useEffect, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, CircularProgress, Stack, Typography } from '@mui/material';
import {
  DefAndMetas,
  MetaValues,
  progressBarSize,
  SimpleResizeAbleEditor,
} from './Common';

import types from './ApiStruct';
import { Variant } from '@mui/material/styles/createTypography';

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  language: string;
  titleVariant?: Variant;
}) {
  let { reloadOP, language, titleVariant } = props;
  if (!titleVariant) {
    titleVariant = 'h3';
  }
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas | null>(null);
  const { showNotification } = useContext(NotificationContext);

  const handleReload = () => {
    setLoading(true);
    reloadOP()
      .then((detail) => {
        setDetail(detail);
        setLoading(false);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'fetch triggre detail failure', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={handleReload} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>
      {detail && (
        <Stack spacing={1}>
          <Typography variant={titleVariant}> Sql </Typography>
          <SimpleResizeAbleEditor
            language={language}
            value={detail.def}
          ></SimpleResizeAbleEditor>
          <Typography variant={titleVariant}> Some Metas </Typography>
          <MetaValues values={detail.metas}></MetaValues>
        </Stack>
      )}
    </Stack>
  );
}

function UdfView(props: {
  reloadOP: () => Promise<types.UdfDetail>;
  language: string;
}) {
  let { reloadOP, language } = props;
  return (
    <DefAndMetasView language={language} reloadOP={reloadOP}></DefAndMetasView>
  );
}

function ProcudureView(props: {
  reloadOP: () => Promise<types.UdfDetail>;
  language: string;
}) {
  let { reloadOP, language } = props;
  return (
    <DefAndMetasView language={language} reloadOP={reloadOP}></DefAndMetasView>
  );
}

export { DefAndMetasView, ProcudureView, UdfView };
