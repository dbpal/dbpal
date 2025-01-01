import { useContext, useEffect, useState } from 'react';

import {
  Button,
  createTheme,
  CssBaseline,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  ThemeProvider,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import {
  appStore,
  AppStoreKey,
  SelectPageSize,
  SettingsContext,
  UserSettings,
} from './Common';

function ModifySettings(props: { handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const { handleClose } = props;
  const [pageSize, setPageSize] = useState(0);
  const [theme, setTheme] = useState('');

  const reload = () => {
    appStore
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((settings) => {
        setPageSize(settings.pageSize);
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
    appStore
      .set(AppStoreKey.UserSetttings, new UserSettings(theme, pageSize))
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

const lightTheme = createTheme();
const darkTheme = createTheme({ palette: { mode: 'dark' } });

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(null);
  const reload = () => {
    appStore
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((s) => {
        if (!s) {
          s = new UserSettings();
        }
        setSetting(s);
      })

      .catch((e) => {
        alert(`list user settings failed`);
        console.log('list user settting failed', e);
      });
  };
  const isDark = () => {
    if (!setting) {
      return false;
    }
    return setting.theme == 'dark';
  };
  appStore.onChange((key) => {
    if (key == AppStoreKey.UserSetttings) {
      reload();
    }
  });
  useEffect(() => {
    reload();
  }, []);

  return (
    <ThemeProvider theme={!isDark() ? lightTheme : darkTheme}>
      <CssBaseline />
      <SettingsContext.Provider value={setting}>
        {children}
      </SettingsContext.Provider>
    </ThemeProvider>
  );
}

export { ModifySettings, SettingComp };
