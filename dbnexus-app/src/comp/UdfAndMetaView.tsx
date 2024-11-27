import { ReactNode, useContext, useEffect, useState } from 'react';

import { NotificationContext } from './Notification';
import {
  Alert,
  Button,
  CircularProgress,
  Stack,
  Typography,
} from '@mui/material';
import { DefAndMetas, MetaValues, SimpleResizeAbleEditor } from './common';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import types from './ApiStruct';
import { Variant } from '@mui/material/styles/createTypography';

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  dropOP: () => Promise<void>;
  language: string;
  dropText: string;
  afterDropMsg: string;
  extra?: (x: DefAndMetas, handleReload: any) => ReactNode;
  titleVariant?: Variant;
}) {
  let {
    reloadOP,
    dropOP,
    language,
    dropText,
    afterDropMsg,
    extra,
    titleVariant,
  } = props;
  if (!titleVariant) {
    titleVariant = 'h3';
  }
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [droped, setDroped] = useState(loading);
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

  const handleDrop = () => {
    setLoading(true);
    dropOP()
      .then(() => {
        setLoading(false);
        setDroped(true);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'drop failed', e);
      });
  };

  return (
    <Stack spacing={1}>
      {droped && (
        <Alert color='error' severity='error'>
          {afterDropMsg}
        </Alert>
      )}

      {loading && <CircularProgress />}
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
          {extra && extra(detail, handleReload)}
        </Stack>
      )}
      <Typography color='error' variant={titleVariant}>
        {dropText}
      </Typography>
      <Stack direction='row'>
        <Button
          variant='contained'
          color='error'
          disabled={droped}
          onClick={() => {
            handleDrop();
          }}
        >
          <DeleteForeverIcon></DeleteForeverIcon>
        </Button>
      </Stack>
    </Stack>
  );
}

function UdfView(props: {
  reloadOP: () => Promise<types.UdfDetail>;
  dropOP: () => Promise<void>;
  language: string;
}) {
  let { reloadOP, dropOP, language } = props;
  return (
    <DefAndMetasView
      language={language}
      dropText='Drop UDF'
      dropOP={dropOP}
      afterDropMsg='The UDF is dropped,please close this tab'
      reloadOP={reloadOP}
    ></DefAndMetasView>
  );
}

function ProcudureView(props: {
  reloadOP: () => Promise<types.UdfDetail>;
  dropOP: () => Promise<void>;
  language: string;
}) {
  let { reloadOP, dropOP, language } = props;
  return (
    <DefAndMetasView
      language={language}
      dropText='Drop Precedure'
      dropOP={dropOP}
      afterDropMsg='The Precedure is dropped,please close this tab'
      reloadOP={reloadOP}
    ></DefAndMetasView>
  );
}

export { DefAndMetasView, ProcudureView, UdfView };
