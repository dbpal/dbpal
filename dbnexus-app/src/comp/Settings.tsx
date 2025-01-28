import { useContext, useEffect, useState } from 'react';

import {
  Button,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
  Theme,
  ThemeProvider,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { appStore, AppStoreKey, SettingsContext, UserSettings } from './Common';
import { SelectPageSize } from './CommonComp';

function ModifySettings() {
  const { showNotification } = useContext(NotificationContext);

  const [pageSize, setPageSize] = useState(0);
  const [theme, setTheme] = useState('');
  const [editorFontSize, setEditorFontSize] = useState('18');
  const reload = async () => {
    (await appStore())
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((settings) => {
        setPageSize(settings.pageSize);
        setTheme(settings.theme);
        setEditorFontSize(settings.editorFontSize + '');
      })
      .catch((e) => {
        showNotification('error', 'port is not a number', e);
      });
  };
  useEffect(() => {
    reload();
  }, []);

  const handleSave = async () => {
    (await appStore())
      .set(
        AppStoreKey.UserSetttings,
        new UserSettings(theme, pageSize, parseInt(editorFontSize))
      )
      .then(() => {
        showNotification('success', 'Save Setting OK');
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };

  const headerSize = '200px';
  return (
    <Stack spacing={1}>
      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> page size: </Typography>
        <SelectPageSize
          pageSize={pageSize}
          handleChange={(size) => {
            setPageSize(size);
          }}
        ></SelectPageSize>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> theme: </Typography>
        <FormControl style={{ width: '200px' }} variant="outlined">
          <InputLabel id="dropdown-label">select a theme</InputLabel>
          <Select
            labelId="dropdown-label"
            value={theme}
            onChange={(e) => {
              setTheme(e.target.value);
            }}
          >
            <MenuItem value="dark">dark</MenuItem>
            <MenuItem value="light">light</MenuItem>
          </Select>
        </FormControl>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> code editor font size: </Typography>
        <TextField
          size="small"
          value={editorFontSize}
          onChange={(e) => {
            setEditorFontSize(e.target.value);
          }}
          error={isNaN(parseInt(editorFontSize))}
          helperText="please input a int"
        ></TextField>
      </Stack>

      <Divider></Divider>
      <Stack direction="row">
        <Button
          variant="contained"
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

const components: Components<
  Omit<Theme, 'components' | 'palette'> & CssVarsTheme
> = {
  MuiButton: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        padding: '3px',
        textTransform: 'none',
      },
    },
  },
  MuiMenuItem: {
    styleOverrides: {
      root: {
        height: '1.7rem',
        fontSize: '1rem',
      },
    },
  },
  //@ts-ignore
  MuiDataGrid: {
    styleOverrides: {
      cell: {
        padding: '0px 2px', // 调整单元格 padding

      },
      columnHeaders: {
        padding: '0px 2px', // 调整表头 padding
      },
    },
  },
  MuiMenuList: {
    styleOverrides: {
      root: {
        padding: '2px 0',
      },
    },
  },
  MuiSelect: {
    styleOverrides: {
      root: {
        height: '2.5rem',
      },
    },
  },
  MuiAlert: {
    styleOverrides: {
      root: {
        padding: 0,
      },
    },
  },
  MuiTabs: {
    styleOverrides: {
      root: {
        height: '2.6rem',
        minHeight: '1rem',
      },
    },
  },
  MuiTab: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        height: '1.2rem',
        textTransform: 'none',
      },
    },
  },
};

const lightTheme = createTheme({ palette: { mode: 'light' }, components });
const darkTheme = createTheme({ palette: { mode: 'dark' }, components });

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(null);
  const reload = async () => {
    (await appStore())
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
  appStore().then((appStore) => {
    appStore.onChange((key) => {
      if (key == AppStoreKey.UserSetttings) {
        reload();
      }
    });
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
