import { invoke } from '@tauri-apps/api/core';
import { useContext, useEffect, useState } from 'react';
import types from './ApiStruct';
import {
  Button,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { SelectPageSize, SettingsContext } from './Common';
import { listen } from '@tauri-apps/api/event';

function ModifySettings(props: { handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const { handleClose } = props;
  const [pageSize, setPageSize] = useState(0);
  const [theme, setTheme] = useState('');

  const reload = () => {
    invoke<types.UserSettings>('get_user_settings')
      .then((settings) => {
        setPageSize(settings.page_size);
        setTheme(settings.theme);
      })
      .catch((e) => {
        showNotification('error', 'port is not a number', e);
      });
  };
  useEffect(() => {
    reload();
  }, []);

  const handleSave = () => {
    invoke('save_user_settings', {
      userSettings: {
        page_size: pageSize,
        theme,
      } as types.UserSettings,
    })
      .then(() => {
        showNotification('success', 'Save Setting OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };

  const headerSize = '100px';
  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> page size: </Typography>
        <SelectPageSize
          pageSize={pageSize}
          handleChange={(size) => {
            setPageSize(size);
          }}
        ></SelectPageSize>
      </Stack>
      <Stack direction='row' spacing={1} sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> theme: </Typography>
        <FormControl style={{ width: '200px' }} size='small' variant='outlined'>
          <InputLabel id='dropdown-label'>select a theme</InputLabel>
          <Select
            labelId='dropdown-label'
            value={theme}
            onChange={(e) => {
              setTheme(e.target.value);
            }}
            label='选择一个选项'
          >
            <MenuItem value='dark'>dark</MenuItem>
            <MenuItem value='light'>light</MenuItem>
          </Select>
        </FormControl>
      </Stack>
      <div></div>
      <Stack direction='row'>
        <Button
          variant='contained'
          onClick={() => {
            handleSave();
          }}
        >
          save
        </Button>
      </Stack>
    </Stack>
  );
}

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<types.UserSettings>(null);
  const { showNotification } = useContext(NotificationContext);
  const reload = () => {
    invoke<types.UserSettings>('get_user_settings')
      .then((s) => {
        setSetting(s);
      })
      .catch((e) => {
        showNotification('error', 'port is not a number', e);
      });
  };

  listen('settings_changed', () => {
    reload();
  });
  useEffect(() => {
    reload();
  }, []);

  return (
    <SettingsContext.Provider value={setting}>
      {children}
    </SettingsContext.Provider>
  );
}

export { ModifySettings, SettingComp };
