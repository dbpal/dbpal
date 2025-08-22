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
  EscDownEvent,
  modalStyle,
  SimpleFlexSizeEditor,
} from './Common';
import { ModalHeader, SelectPageSize } from './CommonComp';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  appFontWeight,
  JsonError,
  SettingContext,
  UserSettings,
  validJson,
} from './helper';
import { Position } from 'monaco-editor';

function ModifySettings({ handleClose }) {
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
    try {
      let settings = await UserSettings.get();
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
    } catch (e) {
      showNotification('error', 'get user config failed', e);
      return;
    }
  };
  useEffect(() => {
    reload();
  }, []);

  const handleSave = async () => {
    try {
      await validJson(editorOptions);
    } catch (e) {
      showNotification('error', 'not a valid json', e);
      return;
    }
    appStore
      .set(
        AppStoreKey.UserSetttings,
        UserSettings.merge({
          theme,
          pageSize,
          editorFontSize,
          rootFontSize,
          datagridRowHeight,
          datagridHeaderHeight,
          editorOption: JSON.parse(editorOptions),
        } as UserSettings)
      )
      .then(() => {
        handleClose();
        showNotification('success', 'Save Setting OK');
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };

  const headerSize = '30%';
  return (
    <Stack spacing={1}>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          default page size:
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
          color="info"
        ></Chip>

        <Chip
          label="dark"
          disabled={theme == 'dark'}
          onClick={() => {
            setTheme('dark');
          }}
          color="info"
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
          table header height:
        </Typography>
        <Chip label={datagridHeaderHeight} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={datagridHeaderHeight}
          onChange={(_, value) => {
            setDatagridHeaderHeight(value as number);
          }}
          min={15}
          max={50}
        ></Slider>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          table row height:
        </Typography>
        <Chip label={datagridRowHeight} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={datagridRowHeight}
          onChange={(_, value) => {
            setDatagridRowHeight(value as number);
          }}
          min={15}
          max={50}
        ></Slider>
      </Stack>
      <Stack spacing={1}>
        <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
          <Tooltip
            title="dbpal use monaco editor as build in editor"
            placement="top"
            arrow
          >
            <Typography width={headerSize} textAlign="right">
              monaco editor options:
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
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowEditorOptions(false);
          }}
        >
          <Box sx={modalStyle} width="75%" height="50vh">
            <ModalHeader title="Editor Options"></ModalHeader>
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
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [jsonErr, setJsonErr] = useState<JsonError>(null);
  useEffect(() => {
    validJson(value)
      .then(() => {
        setJsonErr(null);
      })
      .catch((e) => {
        setJsonErr(e);
      });
  }, [value]);
  return (
    <Stack spacing={1} flex={1}>
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
      <SimpleFlexSizeEditor
        errPos={jsonErrPos}
        language="json"
        value={value}
        handleValue={(v) => {
          setValue(v);
        }}
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              variant="body1"
              style={{ cursor: 'pointer' }}
              onClick={() => {
                if (!jsonErrPos) {
                  return;
                }
                setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
              }}
            >
              {jsonErr.msg}
            </Typography>
          ) : null
        }
      ></SimpleFlexSizeEditor>
      <Divider></Divider>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained"
          onClick={async () => {
            try {
              await validJson(value);
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
        padding: '2px  4px',
        textTransform: 'none',
        fontWeight: appFontWeight,
      },
    },
  },
  MuiIconButton: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        padding: '2px 4px',
        textTransform: 'none',
      },
    },
  },
  MuiMenuItem: {
    styleOverrides: {
      root: {
        height: '1.6rem',
        fontSize: '1rem',
      },
    },
  },
  MuiAlert: {
    styleOverrides: {
      root: {
        fontWeight: appFontWeight,
      },
    },
  },
  MuiTooltip: {
    styleOverrides: {
      tooltip: {
        fontSize: '0.9rem',
      },
      arrow: true,
    },
  },
  MuiAlertTitle: {
    styleOverrides: {
      root: {
        fontWeight: appFontWeight,
      },
    },
  },
  //@ts-ignore
  MuiDataGrid: {
    styleOverrides: {
      root: {
        '& .MuiDataGrid-columnHeaderTitle': {
          fontWeight: appFontWeight,
        },
      },
      cell: {
        fontSize: '1rem',
      },
      columnHeaders: {
        fontSize: '0.9rem',
        boxShadow: '0 2px 4px',
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
  MuiTextField: {
    styleOverrides: {
      root: {
        '& .MuiInputBase-input': {
          fontWeight: appFontWeight,
        },
      },
    },
  },
  MuiChip: {
    styleOverrides: {
      root: {
        fontSize: '1rem',
        fontWeight: appFontWeight,
      },
    },
  },
  MuiTab: {
    styleOverrides: {
      root: {
        textTransform: 'none',
      },
    },
  },
};

const lightTheme = createTheme({
  typography: {
    allVariants: {
      fontWeight: appFontWeight,
    },
  },
  palette: {
    mode: 'light',
    background: {
      default: '#d6d6d6',
      paper: '#d6d6d6',
    },
  },
  components,
});

//@ts-ignore
lightTheme.components.MuiDataGrid.styleOverrides.row = {
  // '&:nth-of-type(odd)': {
  //   backgroundColor: '#ffffff', // 浅色模式的奇数行背景色
  // },
  // '&:nth-of-type(even)': {
  //   backgroundColor: '#d0d0d0', // 浅色模式的偶数行背景色
  // },
};

const darkTheme = createTheme({
  typography: {
    allVariants: {
      fontWeight: appFontWeight,
    },
  },
  palette: {
    mode: 'dark',
    background: {
      default: '#292929',
      paper: '#292929',
    },
  },
  components,
});
//@ts-ignore
darkTheme.components.MuiDataGrid.styleOverrides.row = {
  // '&:nth-of-type(odd)': {
  //   backgroundColor: '#00000', // 深色模式的奇数行背景色
  // },
  // '&:nth-of-type(even)': {
  //   backgroundColor: '#2f2f2f', // 深色模式的偶数行背景色
  // },
};

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(null);
  const reload = async () => {
    UserSettings.get()
      .then((setting) => {
        let x = UserSettings.merge(setting);
        setSetting(x);
        // why new a object because of js is a dynamic language
        // UserSettings maybe have no value on store
        document.documentElement.style.fontSize = x.rootFontSize + 'px';
      })
      .catch((e) => {
        alert(`list user settings failed ${e.toString()}`);
        console.log('list user setting failed', e);
      });
  };
  const isDark = setting && setting.theme == 'dark';

  appStore.onChange((key) => {
    if (key == AppStoreKey.UserSetttings) {
      reload();
    }
  });

  useEffect(() => {
    reload();
  }, []);
  return (
    <ThemeProvider theme={!isDark ? lightTheme : darkTheme}>
      <CssBaseline />
      <SettingContext.Provider value={setting}>
        {children}
      </SettingContext.Provider>
    </ThemeProvider>
  );
}

export { ModifySettings, SettingComp };
