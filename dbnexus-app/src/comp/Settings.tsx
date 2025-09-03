import { CSSProperties, useContext, useEffect, useState } from 'react';

import {
  Button,
  Chip,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  IconButton,
  MenuItem,
  Select,
  Slider,
  Stack,
  TextField,
  Theme,
  ThemeProvider,
  Tooltip,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { appStore, AppStoreKey, SimpleFlexSizeEditor } from './Common';
import { SelectPageSize } from './CommonComp';
import { openUrl } from '@tauri-apps/plugin-opener';
import { ErrAndLoc, SettingContext, UserSettings, validJson } from './helper';
import { Position } from 'monaco-editor';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import { debugBuild } from './debug';

function ModifySettings({ handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(0);
  const [fontWeight, setFontWeight] = useState('normal');
  const [fontSize, setFontSize] = useState(16);
  const [editorOptions, setEditorOptions] = useState('');
  const [showEditorOptions, setShowEditorOptions] = useState(false);
  const [fontFamily, setFontFamily] = useState<string>('');
  const [allFonts, setAllFonts] = useState<string[]>([]);
  const [fontMatch, setFontMatch] = useState<string>('');
  const [tabSize, setTabSize] = useState(2);
  const [theme, setTheme] = useState('');
  useEffect(() => {
    invoke<string[]>('fonts_all_families', {})
      .then((fonts) => {
        setAllFonts(fonts);
      })
      .catch((e) => {
        showNotification('error', 'list all fonts failed', e);
      });
  }, []);
  const setting = useContext(SettingContext);
  const reload = async () => {
    try {
      let settings = await UserSettings.get();
      setPageSize(settings.pageSize);
      setFontWeight(settings.fontWeight);
      setFontSize(settings.fontSize);
      setFontFamily(settings.fontFamily);
      setTabSize(settings.tabSize);
      setTheme(setting.theme);
      const content = JSON.stringify(
        settings.editorOption ? settings.editorOption : {},
        null,
        setting.tabSize
      );
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
      const e = await validJson(editorOptions);
      if (e) {
        showNotification('error', 'not a valid json', e.msg);
        return;
      }
    } catch (e) {
      showNotification('error', 'call valid json error', e);
      return;
    }
    UserSettings.set({
      fontWeight,
      pageSize,
      fontSize,
      fontFamily,
      tabSize,
      theme,
      editorOption: JSON.parse(editorOptions),
    })
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
      {debugBuild && (
        <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
          <Typography width={headerSize} textAlign="right">
            theme:
          </Typography>
          {['ligth', 'dark'].map((e) => (
            <Chip
              label={e}
              color={theme == e ? 'primary' : 'default'}
              onClick={() => {
                setTheme(e);
              }}
            ></Chip>
          ))}
        </Stack>
      )}
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          font size:
        </Typography>
        <Chip label={fontSize} style={{ fontSize }} />
        <Slider
          style={{ width: '30%' }}
          size="small"
          value={fontSize}
          onChange={(_, value) => {
            setFontSize(value as number);
          }}
          min={12}
          max={25}
        ></Slider>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right" style={{ fontWeight }}>
          font weight:
        </Typography>
        <TextField
          value={fontWeight}
          onChange={(e) => {
            setFontWeight(e.target.value);
          }}
          size="small"
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography
          width={headerSize}
          style={{ fontFamily: fontFamily }}
          textAlign="right"
        >
          font family:
        </Typography>

        <Select
          size="small"
          value={fontFamily}
          onChange={(e) => setFontFamily(e.target.value as string)}
          style={{ fontFamily: fontFamily, width: '200px' }}
        >
          <MenuItem style={{ fontFamily: undefined }} value="">
            default
          </MenuItem>
          {allFonts
            .filter((f) => {
              if (fontMatch) {
                return f.toLowerCase().includes(fontMatch.toLowerCase());
              } else {
                return true;
              }
            })
            .map((f) => {
              return (
                <MenuItem style={{ fontFamily: f }} value={f}>
                  {f}
                </MenuItem>
              );
            })}
        </Select>
        <TextField
          size="small"
          label="match font"
          value={fontMatch}
          style={{ fontFamily: fontFamily }}
          onChange={(e) => {
            setFontMatch(e.target.value);
          }}
        ></TextField>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          tab size:
        </Typography>

        {[2, 4, 8].map((e) => (
          <Chip
            label={e}
            color={e == tabSize ? 'primary' : 'default'}
            onClick={() => {
              setTabSize(e);
            }}
          ></Chip>
        ))}
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
          <IconButton
            onClick={() => {
              setShowEditorOptions((e) => !e);
            }}
            color="primary"
          >
            <EditIcon> </EditIcon>
          </IconButton>
          <Typography color="info">{editorOptions}</Typography>
        </Stack>
      </Stack>

      {editorOptions && (
        <EditorOptions
          handleClose={() => {
            setShowEditorOptions(false);
          }}
          style={{
            height: '40vh',
            border: '1px solid',
            borderRadius: '8px',
            padding: '8px',
            display: showEditorOptions ? 'flex' : 'none',
          }}
          editorOptions={editorOptions}
          setEditorOptions={setEditorOptions}
        ></EditorOptions>
      )}

      <Divider />
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

function EditorOptions(props: {
  editorOptions;
  setEditorOptions;
  handleClose;
  style: CSSProperties;
}) {
  const { showNotification } = useContext(NotificationContext);
  const { editorOptions, setEditorOptions, handleClose, style } = props;
  const [value, setValue] = useState(editorOptions);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  useEffect(() => {
    validJson(value)
      .then((e) => {
        setJsonErr(e);
      })
      .catch((e) => {
        console.log('call valid json error', e);
      });
  }, [value]);
  return (
    <Stack spacing={1} style={style}>
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
      <Divider />
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
          set
        </Button>
      </Stack>
    </Stack>
  );
}

function makeComponents(appFontWeight) {
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
          height: '1.7rem',
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
  return components;
}

const lightTheme = (appFontWeight, fontFamily) => {
  return createTheme({
    typography: {
      allVariants: {
        fontWeight: appFontWeight,
      },
      fontFamily,
    },
    palette: {
      mode: 'light',
      background: {
        default: '#d6d6d6',
        paper: '#d6d6d6',
      },
    },
    components: makeComponents(appFontWeight),
  });
};

const darkTheme = (appFontWeight, fontFamily) => {
  return createTheme({
    typography: {
      allVariants: {
        fontWeight: appFontWeight,
      },
      fontFamily,
    },
    palette: {
      mode: 'dark',
      background: {
        default: '#292929',
        paper: '#292929',
      },
    },
    components: makeComponents(appFontWeight),
  });
};

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(null);
  const reload = async () => {
    UserSettings.get()
      .then((setting) => {
        setSetting(setting);
        console.log('setting', setting);
        // why new a object because of js is a dynamic language
        // UserSettings maybe have no value on store
        document.documentElement.style.fontSize = setting.fontSize + 'px';
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
    <ThemeProvider
      theme={
        !isDark
          ? lightTheme(
              setting && setting.fontWeight,
              setting && setting.fontFamily
            )
          : darkTheme(
              setting && setting.fontWeight,
              setting && setting.fontFamily
            )
      }
    >
      <CssBaseline />
      <SettingContext.Provider value={setting}>
        {children}
      </SettingContext.Provider>
    </ThemeProvider>
  );
}

export { ModifySettings, SettingComp };
