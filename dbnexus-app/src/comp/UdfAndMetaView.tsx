import { useContext, useEffect, useState } from 'react';

import { NotificationContext } from './Notification';
import { Button, Stack, Typography } from '@mui/material';
import { DefAndMetas, SettingsContext, SimpleResizeEditor } from './Common';

import { Variant } from '@mui/material/styles/createTypography';
import { MetaValuesView } from './CommonComp';

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
        showNotification('error', 'fetch detail failed', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  const { theme } = useContext(SettingsContext);
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
          <Typography variant={titleVariant}> Sql </Typography>
          <SimpleResizeEditor
            style={{ minHeight: '200px' }}
            language={language}
            value={detail.def}
            onMount={(_editor, monaco) => {
              if (theme == 'dark') {
                monaco.editor.setTheme('vs-dark');
              }
            }}
          ></SimpleResizeEditor>
          <Typography variant={titleVariant}> Some Metas </Typography>
          <MetaValuesView values={detail.metas}></MetaValuesView>
        </div>
      )}
    </Stack>
  );
}

export { DefAndMetasView };
