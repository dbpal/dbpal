import { CSSProperties, useContext, useEffect, useRef, useState } from 'react';

import {
  Button,
  Chip,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  IconButton,
  InputAdornment,
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
import { HexColorPicker } from 'react-colorful';
import { NotificationContext } from './Notification';
import { appStore, AppStoreKey, SimpleFlexSizeEditor } from './Common';
import { SelectPageSize } from './CommonComp';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  ErrAndLoc,
  isColor,
  isLightHex,
  SettingContext,
  UserSettings,
  validJson,
} from './helper';
import { Position } from 'monaco-editor';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import ClearIcon from '@mui/icons-material/Clear';
function ModifySettings() {
  const { showNotification } = useContext(NotificationContext);

  const [pageSize, setPageSize] = useState(0);
  const [fontWeight, setFontWeight] = useState('normal');
  const [fontSize, setFontSize] = useState(16);
  const [editorOptions, setEditorOptions] = useState('');
  const [showEditorOptions, setShowEditorOptions] = useState(false);
  const [showEditBgColor, setShowEditBgColor] = useState(false);
  const [fontFamily, setFontFamily] = useState<string>('');
  const [allFonts, setAllFonts] = useState<string[]>([]);
  const [fontMatch, setFontMatch] = useState<string>('');
  const [tabSize, setTabSize] = useState(2);
  const [bgColor, setBgColor] = useState('');
  const [bgColorErr, setBgColorErr] = useState('');
  const bgColorTextField = useRef<HTMLInputElement>(null);
  useEffect(() => {
    invoke<string[]>('fonts_all_families', {})
      .then((fonts) => {
        setAllFonts(fonts);
      })
      .catch((e) => {
        showNotification('error', 'list all fonts failed', e);
      });
  }, []);

  const reload = async () => {
    try {
      const setting = await UserSettings.get();
      setPageSize(setting.pageSize);
      setFontWeight(setting.fontWeight);
      setFontSize(setting.fontSize);
      setFontFamily(setting.fontFamily);
      setTabSize(setting.tabSize);
      if (setting.isDark()) {
        setBgColor(setting.darkBgColor);
      } else {
        setBgColor(setting.lightBgColor);
      }
      const content = JSON.stringify(
        setting.editorOption ? setting.editorOption : {},
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
  let theme = useContext(SettingContext).theme;
  const handleSave = async () => {
    if (bgColorErr) {
      bgColorTextField.current.focus();
      return;
    }
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
    let s = {
      fontWeight,
      pageSize,
      fontSize,
      fontFamily,
      tabSize,
      editorOption: JSON.parse(editorOptions),
    } as UserSettings;
    if (theme == 'dark') {
      s.darkBgColor = bgColor;
    } else {
      s.lightBgColor = bgColor;
    }
    UserSettings.set(s)
      .then(() => {
        showNotification('success', 'Save Setting OK');
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };
  const headerSize = '30%';
  useEffect(() => {
    if (!bgColor) {
      setBgColorErr('');
      return;
    }
    if (!isColor(bgColor)) {
      setBgColorErr('not a valid color');
      return;
    }
    if (
      (theme == 'dark' && isLightHex(bgColor)) ||
      (theme == 'light' && !isLightHex(bgColor))
    ) {
      setBgColorErr(`please set a ${theme} background color`);
      return;
    }
    setBgColorErr('');
  }, [bgColor]);

  return (
    <Stack
      spacing={1}
      style={{ backgroundColor: bgColorErr ? undefined : bgColor }}
    >
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
          slotProps={{
            input: {
              endAdornment: fontWeight && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFontWeight('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
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
          slotProps={{
            input: {
              endAdornment: fontMatch && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFontMatch('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
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
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          background color:
        </Typography>
        <div style={{ position: 'relative' }}>
          <IconButton
            color="primary"
            onClick={() => {
              setShowEditBgColor((e) => !e);
            }}
          >
            <EditIcon> </EditIcon>
          </IconButton>
          {showEditBgColor && (
            <HexColorPicker
              color={bgColor}
              onChange={(newValue) => {
                setBgColor(newValue);
              }}
              style={{ position: 'absolute', zIndex: 10, top: '3rem' }}
            />
          )}
        </div>

        <TextField
          value={bgColor}
          onChange={(e) => {
            setBgColor(e.target.value);
          }}
          inputRef={bgColorTextField}
          style={{ width: '10rem' }}
          size="small"
          error={!!bgColorErr}
          slotProps={{
            input: {
              endAdornment: bgColor && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setBgColor('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        ></TextField>
        <div style={{ position: 'relative' }}></div>
        {bgColorErr && <Typography color="error"> {bgColorErr} </Typography>}
      </Stack>

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

const lightTheme = (appFontWeight, fontFamily, bgColor) => {
  const color = bgColor ? bgColor : '#fff';

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
        default: color,
        paper: color,
      },
    },
    components: makeComponents(appFontWeight),
  });
};

const darkTheme = (appFontWeight, fontFamily, bgColor) => {
  const color = bgColor ? bgColor : '#000';

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
        default: color,
        paper: color,
      },
    },
    components: makeComponents(appFontWeight),
  });
};

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSettings>(new UserSettings());
  const reload = async () => {
    UserSettings.get()
      .then((setting) => {
        setSetting(setting);
        document.documentElement.style.fontSize = setting.fontSize + 'px';
      })
      .catch((e) => {
        alert(`list user settings failed ${e.toString()}`);
        console.log('list user setting failed', e);
      });
  };
  appStore.onChange((key) => {
    if (key == AppStoreKey.UserSettings) {
      reload();
    }
  });
  useEffect(() => {
    reload();
  }, []);
  useEffect(() => {}, [setting]);
  return (
    <ThemeProvider
      theme={
        setting.isDark()
          ? darkTheme(
              setting.fontWeight,
              setting.fontFamily,
              setting.darkBgColor
            )
          : lightTheme(
              setting.fontWeight,
              setting.fontFamily,
              setting.lightBgColor
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
