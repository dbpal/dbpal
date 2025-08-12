import { useContext, useEffect, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, Stack, Typography } from '@mui/material';
import { DefAndMetas, SimpleFlexSizeEditor } from './Common';
import { MetaValuesView } from './CommonComp';
import { saveAndSetSize } from './helper';
``;
function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  language: string;
  showReload?: boolean;
  heightName?: string;
}) {
  let { reloadOP, language, showReload, heightName } = props;
  const titleVariant = 'h6';
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas>({
    def: '',
    metas: [],
  });
  const { showNotification } = useContext(NotificationContext);

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
    <Stack spacing={1}>
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
      </Stack>
      <Typography variant={titleVariant}> Statement </Typography>
      <div
        ref={setDom}
        style={{
          display: 'flex',
          flexDirection: 'column',
          overflow: 'auto',
          height: '300px',
          minHeight: '20vh',
          resize: 'vertical',
        }}
      >
        <SimpleFlexSizeEditor
          language={language}
          value={detail.def}
          readOnly={true}
        ></SimpleFlexSizeEditor>
      </div>
      {detail.metas.length > 0 && (
        <>
          <Typography variant={titleVariant}> Some Metas </Typography>
          <MetaValuesView values={detail.metas}></MetaValuesView>{' '}
        </>
      )}
    </Stack>
  );
}

export { DefAndMetasView };
