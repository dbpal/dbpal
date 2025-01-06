import { Button, Stack } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';
import types from './ApiStruct';
import { getNamedConfig, MetaValuesView } from './Common';
import { useContext, useEffect, useState } from 'react';
import { NotificationContext } from './Notification';

function ConnInfo(props: { uuid }) {
  let { uuid } = props;
  const { showNotification } = useContext(NotificationContext);
  const [metas, setMetas] = useState<types.MetaValue[]>([]);
  const handleInfo = async () => {
    try {
      let metas = await invoke<types.MetaValue[]>('server_metas', {
        name: uuid,
      });
      let config = await getNamedConfig(uuid);
      let configMetas = [];
      switch (config.conn_type) {
        case 'mysql':
        case 'mariadb':
        case 'postgres':
        case 'redis':
        case 'mongo':
          configMetas = [
            { name: 'host', value: config.host } as types.MetaValue,
            { name: 'port', value: config.port + '' } as types.MetaValue,
            { name: 'user', value: config.username } as types.MetaValue,
          ];
          break;
        case 'sqlite':
          configMetas = [
            { name: 'host', value: config.path } as types.MetaValue,
          ];
          break;
      }
      setMetas([...configMetas, ...metas]);
    } catch (e) {
      showNotification('error', 'call server_metas failed', e);
    }
  };
  useEffect(() => {
    handleInfo();
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Button variant="contained" color="primary" onClick={handleInfo}>
          reload
        </Button>
      </Stack>
      <div style={{ display: 'grid' }}>
        <MetaValuesView values={metas}></MetaValuesView>
      </div>
    </Stack>
  );
}

export { ConnInfo };
