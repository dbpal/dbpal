import { useContext, useEffect, useState } from 'react';

import {
  Box,
  Button,
  Chip,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  Modal,
  Slider,
  Stack,
  Theme,
  ThemeProvider,
  Tooltip,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import {
  appStore,
  AppStoreKey,
  modalStyle,
  SettingContext,
  SimpleResizeEditor,
  UserSettings,
} from './Common';
import { SelectPageSize } from './CommonComp';
import { openUrl } from '@tauri-apps/plugin-opener';

function ModifySettings() {
  const { showNotification } = useContext(NotificationContext);

  const [pageSize, setPageSize] = useState(0);
  const [theme, setTheme] = useState('');
  const [editorFontSize, setEditorFontSize] = useState(18);
  const [rootFontSize, setRootFontSize] = useState(16);
  const [datagridRowHeight, setDatagridRowHeight] = useState(24);
  const [datagridHeaderHeight, setDatagridHeaderHeight] = useState(30);
  const [editorOptions, setEditorOptions] = useState('{\n}');
  const [showEditorOptions, setShowEditorOptions] = useState(false);
  const reload = async () => {
    (await appStore())
      .get<UserSettings>(AppStoreKey.UserSetttings)
      .then((settings) => {
        setPageSize(settings.pageSize);
        setTheme(settings.theme);
        setEditorFontSize(settings.editorFontSize);
        setRootFontSize(settings.rootFontSize);
        setDatagridRowHeight(settings.datagridRowHeight);
        setDatagridHeaderHeight(settings.datagridHeaderHeight);
        const content = JSON.stringify(settings.editorOption, null, 2);
        if (content) {
          setEditorOptions(content);
        }
      })
      .catch((e) => {
        showNotification('error', 'port is not a number', e);
      });
  };
  useEffect(() => {
    reload();
  }, []);

  const handleSave = async () => {
    let eo;
    try {
      eo = JSON.parse(editorOptions);
    } catch (e) {
      showNotification('error', 'not a valid json', e);
      return;
    }
    (await appStore())
      .set(
        AppStoreKey.UserSetttings,
        new UserSettings(
          theme,
          pageSize,
          editorFontSize,
          rootFontSize,
          datagridRowHeight,
          datagridHeaderHeight,
          eo
        )
      )
      .then(() => {
        showNotification('success', 'Save Setting OK');
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };

  const headerSize = '15%';
  return (
    <Stack spacing={1}>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          page size:
        </Typography>
        <SelectPageSize
          pageSize={pageSize}
          handleChange={(size) => {
            setPageSize(size);
          }}
        ></SelectPageSize>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          theme:
        </Typography>
        <Chip
          label="light"
          disabled={theme == 'light'}
          onClick={() => {
            setTheme('light');
          }}
          style={{ backgroundColor: 'white', color: 'black' }}
        ></Chip>

        <Chip
          label="dark"
          disabled={theme == 'dark'}
          onClick={() => {
            setTheme('dark');
          }}
          style={{ backgroundColor: 'black', color: 'white' }}
        ></Chip>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          code editor font size:
        </Typography>
        <Chip label={editorFontSize} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={editorFontSize}
          onChange={(_, value) => {
            setEditorFontSize(value as number);
          }}
          min={12}
          max={25}
        ></Slider>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          font size:
        </Typography>
        <Chip label={rootFontSize} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={rootFontSize}
          onChange={(_, value) => {
            setRootFontSize(value as number);
          }}
          min={12}
          max={25}
        ></Slider>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          table header size:
        </Typography>
        <Chip label={datagridHeaderHeight} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={datagridHeaderHeight}
          onChange={(_, value) => {
            setDatagridHeaderHeight(value as number);
          }}
          min={20}
          max={50}
        ></Slider>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          table row size:
        </Typography>
        <Chip label={datagridRowHeight} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={datagridRowHeight}
          onChange={(_, value) => {
            setDatagridRowHeight(value as number);
          }}
          min={20}
          max={50}
        ></Slider>
      </Stack>
      <Stack spacing={1}>
        <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
          <Tooltip title="dbpal use monaco editor as build in editor">
            <Typography width={headerSize} textAlign="right">
              editor options:
            </Typography>
          </Tooltip>
          <Typography
            color="info"
            style={{ cursor: 'pointer' }}
            onClick={() => {
              setShowEditorOptions(true);
            }}
          >
            {editorOptions}
          </Typography>
        </Stack>
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

      {showEditorOptions && (
        <Modal
          open={showEditorOptions}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowEditorOptions(false);
          }}
        >
          <Box sx={modalStyle} width="950px">
            <EditorOptions
              handleClose={() => {
                setShowEditorOptions(false);
              }}
              editorOptions={editorOptions}
              setEditorOptions={setEditorOptions}
            ></EditorOptions>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function EditorOptions(props: {
  editorOptions;
  setEditorOptions;
  handleClose;
}) {
  const { showNotification } = useContext(NotificationContext);
  const { editorOptions, setEditorOptions, handleClose } = props;
  const [value, setValue] = useState(editorOptions);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Chip
          color="info"
          style={{ cursor: 'pointer' }}
          label="options???"
          onClick={() => {
            openUrl(
              'https://microsoft.github.io/monaco-editor/typedoc/variables/editor.EditorOptions.html'
            );
          }}
        ></Chip>
      </Stack>
      <SimpleResizeEditor
        language="json"
        value={value}
        onMount={(editor) => {
          editor.onDidChangeModelContent(() => {
            setValue(editor.getValue());
          });
        }}
      ></SimpleResizeEditor>
      <Divider></Divider>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained"
          onClick={() => {
            try {
              JSON.parse(value);
            } catch (e) {
              showNotification('error', 'not a valid json', e);
              return;
            }
            setEditorOptions(value);
            handleClose();
          }}
        >
          modify
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
        padding: '0px',
      },
    },
  },
  MuiChip: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        fontWeight: '600',
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
  // '&.Mui-selected': {
  //   backgroundColor: '#1976d2', // 蓝色背景
  //   color: '#ffffff', // 白色字体
  //   '&:hover': {
  //     backgroundColor: '#115293', // 悬停时的背景色
  //   },
  // },
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
            setting.datagridHeaderHeight,
            setting.editorOption
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
