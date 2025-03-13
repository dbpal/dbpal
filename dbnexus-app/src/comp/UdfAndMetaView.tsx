import { useContext, useEffect, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, Stack, Typography } from '@mui/material';
import { DefAndMetas, SimpleResizeEditor } from './Common';

import { Variant } from '@mui/material/styles/createTypography';
import { MetaValuesView } from './CommonComp';

function DefAndMetasView(props: {
  reloadOP: () => Promise<DefAndMetas>;
  uuid: string;
  titleVariant?: Variant;
}) {
  let { reloadOP, uuid: language, titleVariant } = props;
  if (!titleVariant) {
    titleVariant = 'h4';
  }
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<DefAndMetas | null>(null);
  const { showNotification } = useContext(NotificationContext);

  const handleReload = () => {
    setLoading(true);
    reloadOP()
      .then((detail) => {
        setDetail(detail);
      })
      .catch((e) => {
        showNotification('error', 'fetch detail failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          variant="contained"
          color="primary"
          onClick={handleReload}
        >
          reload
        </Button>
      </Stack>
      {detail && (
        <div style={{ display: 'grid' }}>
          <Typography variant={titleVariant}> Statement </Typography>
          <SimpleResizeEditor
            style={{ minHeight: '200px' }}
            language={language}
            value={detail.def}
          ></SimpleResizeEditor>
          {detail.metas.length > 0 && (
            <>
              <Typography variant={titleVariant}> Some Metas </Typography>
              <MetaValuesView values={detail.metas}></MetaValuesView>
            </>
          )}
        </div>
      )}
    </Stack>
  );
}

export { DefAndMetasView };
