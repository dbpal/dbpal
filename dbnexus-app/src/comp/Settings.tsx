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
import { appStore, AppStoreKey, SettingContext, UserSettings } from './Common';
import { SelectPageSize } from './CommonComp';

function ModifySettings() {
  const { showNotification } = useContext(NotificationContext);

  const [pageSize, setPageSize] = useState(0);
  const [theme, setTheme] = useState('');
  const [editorFontSize, setEditorFontSize] = useState('18');
  const [rootFontSize, setRootFontSize] = useState('18');
  const [datagridRowHeight, setDatagridRowHeight] = useState('24');
  const [datagridHeaderHeight, setDatagridHeaderHeight] = useState('30');

  const reload = async () => {
    (await appStore())
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((settings) => {
        setPageSize(settings.pageSize);
        setTheme(settings.theme);
        setEditorFontSize(settings.editorFontSize + '');
        setRootFontSize(settings.rootFontSize + '');
        setDatagridRowHeight(settings.datagridRowHeight + '');
        setDatagridHeaderHeight(settings.datagridHeaderHeight + '');
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
        new UserSettings(
          theme,
          pageSize,
          parseInt(editorFontSize),
          parseInt(rootFontSize),
          parseInt(datagridRowHeight),
          parseInt(datagridHeaderHeight)
        )
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
          type="number"
          onChange={(e) => {
            setEditorFontSize(e.target.value);
          }}
          error={isNaN(parseInt(editorFontSize))}
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> font size: </Typography>
        <TextField
          size="small"
          value={rootFontSize}
          onChange={(e) => {
            setRootFontSize(e.target.value);
          }}
          type="number"
          error={isNaN(parseInt(rootFontSize))}
          helperText="overall font size"
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> table header size: </Typography>
        <TextField
          size="small"
          value={datagridHeaderHeight}
          type="number"
          onChange={(e) => {
            setDatagridHeaderHeight(e.target.value);
          }}
          error={isNaN(parseInt(datagridHeaderHeight))}
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}> table row size: </Typography>
        <TextField
          size="small"
          value={datagridRowHeight}
          onChange={(e) => {
            setDatagridRowHeight(e.target.value);
          }}
          type="number"
          error={isNaN(parseInt(datagridRowHeight))}
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
        padding: '2px 4px',
        textTransform: 'none',
        fontWeight: 'bold',
      },
    },
  },
  MuiIconButton: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        padding: '2px 4px',
        textTransform: 'none',
        fontWeight: 'bold',
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
        fontSize: '1rem',
        fontWeight: '700',
      },
      columnHeaders: {
        fontSize: '0.8rem', // 调整表头字体大小
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
        height: '2.3rem',
        fontSize: '1rem',
      },
    },
  },
  MuiTextField: {
    styleOverrides: {
      root: {
        '& .MuiInputBase-root': {
          fontSize: '1rem',
        },
        '& .MuiInputLabel-root': {
          fontSize: '1rem',
        },
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
const rowSelected = {
  '&.Mui-selected': {
    backgroundColor: '#1976d2', // 蓝色背景
    color: '#ffffff', // 白色字体
    '&:hover': {
      backgroundColor: '#115293', // 悬停时的背景色
    },
  },
};

//@ts-ignore
lightTheme.components.MuiDataGrid.styleOverrides.row = {
  // '&:nth-of-type(odd)': {
  //   backgroundColor: '#ffffff', // 浅色模式的奇数行背景色
  // },
  // '&:nth-of-type(even)': {
  //   backgroundColor: '#d0d0d0', // 浅色模式的偶数行背景色
  // },
  ...rowSelected,
};

const darkTheme = createTheme({ palette: { mode: 'dark' }, components });
//@ts-ignore
darkTheme.components.MuiDataGrid.styleOverrides.row = {
  // '&:nth-of-type(odd)': {
  //   backgroundColor: '#00000', // 深色模式的奇数行背景色
  // },
  // '&:nth-of-type(even)': {
  //   backgroundColor: '#2f2f2f', // 深色模式的偶数行背景色
  // },
  ...rowSelected,
};

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(null);
  const reload = async () => {
    (await appStore())
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((setting) => {
        setSetting(
          // why new a object because of js is a dynamic language
          // UserSettings maybe have no value on store
          new UserSettings(
            setting.theme,
            setting.pageSize,
            setting.editorFontSize,
            setting.rootFontSize,
            setting.datagridRowHeight,
            setting.datagridHeaderHeight
          )
        );

        document.documentElement.style.fontSize =
          (setting.rootFontSize ? setting.rootFontSize : 15) + 'px';
      })
      .catch((e) => {
        alert(`list user settings failed ${e.toString()}`);
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
      <SettingContext.Provider value={setting}>
        {children}
      </SettingContext.Provider>
    </ThemeProvider>
  );
}

export { ModifySettings, SettingComp };
