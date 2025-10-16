'use strict';
import {
  CSSProperties,
  ReactNode,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';

import {
  Button,
  Chip,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  MenuItem,
  Select,
  Slider,
  Stack,
  Switch,
  TextField,
  Theme,
  ThemeProvider,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import { HexColorPicker } from 'react-colorful';
import { NotificationContext } from './Notification';
import { AppStoreKey } from './Common';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  ErrAndLoc,
  ColorFn,
  SettingContext,
  UserSetting,
  validJson,
  textEllipsisStyle,
  appStore,
  registeredThemes,
} from './helper';
import { Position } from 'monaco-editor';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import ClearIcon from '@mui/icons-material/Clear';
import { SimpleFlexEditor } from './CommonComp2';
import {
  enable as enableApp,
  isEnabled as isEnabledApp,
  disable as disableApp,
} from '@tauri-apps/plugin-autostart';
import { message } from '@tauri-apps/plugin-dialog';
import InfoIcon from '@mui/icons-material/Info';
import { Editor } from '@monaco-editor/react';
function ModifySetting() {
  const { showNotification } = useContext(NotificationContext);
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
  const [csvDelimiter, setCsvDelimiter] = useState(',');
  const [includeHeader, setIncludeHeader] = useState(true);
  const bgColorTextField = useRef<HTMLInputElement>(null);
  const [datagridRowHeightFactor, setDatagridRowHeightFactor] = useState('2');
  const [maxTreeSize, setMaxTreeSize] = useState('50');
  const [maxTabTitleLength, setMaxTabTitleLength] = useState('12');
  const [autoStart, setAutoStart] = useState(false);
  const [customEditorTheme, setCustomEditorTheme] = useState('');
  const setting = useContext(SettingContext);
  useEffect(() => {
    invoke<string[]>('all_font_family', {})
      .then((fonts) => {
        setAllFonts(fonts);
      })
      .catch((e) => {
        showNotification('error', 'list all fonts failed', e);
      });
  }, []);
  const reload = async () => {
    try {
      setFontWeight(setting.fontWeight);
      setFontSize(setting.fontSize);
      setFontFamily(setting.fontFamily);
      setTabSize(setting.tabSize);
      if (setting.isDark()) {
        setBgColor(setting.darkBgColor);
      } else {
        setBgColor(setting.lightBgColor);
      }
      setCsvDelimiter(setting.csvDelimiter);
      setIncludeHeader(setting.includeHeader);
      setMaxTreeSize(setting.maxTreeSize.toString());
      setDatagridRowHeightFactor(setting.datagridRowHeightFactor.toString());
      setMaxTabTitleLength(setting.maxTabTitleLength + '');
      setAutoStart(setting.autoStart);
      setCustomEditorTheme(setting.customEditorTheme);
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

  const handleSave = async () => {
    if (bgColorErr) {
      bgColorTextField.current.focus();
      return;
    }
    if (isNaN(parseFloat(datagridRowHeightFactor))) {
      showNotification('error', 'datagrid row height factor must be a number');
      return;
    }
    if (isNaN(parseInt(maxTreeSize))) {
      showNotification('error', 'max tree size must be a number');
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
    const s = {
      fontWeight,
      fontSize,
      fontFamily,
      tabSize,
      editorOption: JSON.parse(editorOptions),
      csvDelimiter: csvDelimiter,
      includeHeader: includeHeader,
      datagridRowHeightFactor: parseFloat(datagridRowHeightFactor),
      maxTreeSize: parseInt(maxTreeSize),
      theme: setting.theme,
      maxTabTitleLength: parseInt(maxTabTitleLength),
      autoStart,
      customEditorTheme,
    } as UserSetting;
    if (setting.theme == 'dark') {
      s.darkBgColor = bgColor;
    } else {
      s.lightBgColor = bgColor;
    }
    UserSetting.set(s)
      .then(() => {
        showNotification('success', 'Save Setting OK');
      })
      .catch((e) => {
        showNotification('success', 'Save Setting Failure', e);
      });
  };
  const headerSize = '16rem';
  useEffect(() => {
    if (!bgColor) {
      setBgColorErr('');
      return;
    }
    if (!ColorFn.isColor(bgColor)) {
      setBgColorErr('not a valid color');
      return;
    }
    if (
      (setting.theme == 'dark' && ColorFn.isLightHex(bgColor)) ||
      (setting.theme == 'light' && !ColorFn.isLightHex(bgColor))
    ) {
      setBgColorErr(`please set a ${setting.theme} background color`);
      return;
    }
    setBgColorErr('');
  }, [bgColor]);
  const theme = useTheme();

  return (
    <Stack
      spacing={1}
      style={{ backgroundColor: bgColorErr ? undefined : bgColor }}
    >
      <Divider textAlign="left">appearance</Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right" style={{ fontSize }}>
          font size:
        </Typography>
        <Chip label={fontSize} style={{ fontSize }} />
        <Slider
          style={{ width: '30%' }}
          value={fontSize}
          onChange={(_, value) => {
            setFontSize(value as number);
          }}
          min={10}
          max={30}
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
          slotProps={{
            input: {
              endAdornment: fontWeight && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFontWeight('');
                    }}
                    edge="end"
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
        <Typography width={headerSize} textAlign="right" style={{ fontFamily }}>
          font family:
        </Typography>
        {allFonts.length > 0 && (
          <Select
            onChange={(e) => setFontFamily(e.target.value as string)}
            style={{ minWidth: '12rem' }}
            value={fontFamily}
          >
            <MenuItem style={{ fontFamily: undefined }} value=""></MenuItem>
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
                  <MenuItem style={{ fontFamily: f }} value={f} key={f}>
                    {f}
                  </MenuItem>
                );
              })}
          </Select>
        )}

        <TextField
          label="match font"
          value={fontMatch}
          style={{ width: '13rem', fontFamily }}
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
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        ></TextField>
        <Typography
          color="success"
          style={{ ...textEllipsisStyle, fontFamily }}
        >
          select * from employees
        </Typography>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          table row height factor:
        </Typography>
        <TextField
          value={datagridRowHeightFactor}
          onChange={(e) => {
            setDatagridRowHeightFactor(e.target.value);
          }}
          error={isNaN(parseFloat(datagridRowHeightFactor))}
          helperText=""
        ></TextField>
        <Tooltip title="a float value,this parameter multiply font size as the table row height">
          <Typography> ?? </Typography>
        </Tooltip>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          background color:
        </Typography>
        <div style={{ position: 'relative' }}>
          <IconButton
            color={showEditBgColor ? 'success' : 'primary'}
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
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          max tree size:
        </Typography>
        <TextField
          value={maxTreeSize}
          onChange={(e) => {
            setMaxTreeSize(e.target.value);
          }}
          label="max tree item size"
          error={isNaN(parseInt(maxTreeSize))}
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          max tab title length:
        </Typography>
        <TextField
          value={maxTabTitleLength}
          onChange={(e) => {
            setMaxTabTitleLength(e.target.value);
          }}
          error={isNaN(parseInt(maxTabTitleLength))}
        ></TextField>
      </Stack>
      <Divider textAlign="left" style={{ marginTop: '20px' }}>
        editor
      </Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          tab size:
        </Typography>

        {[2, 4, 8].map((e) => (
          <Chip
            label={e}
            color={e == tabSize ? 'success' : 'default'}
            onClick={() => {
              setTabSize(e);
            }}
            key={e}
          ></Chip>
        ))}
      </Stack>
      <Stack spacing={1}>
        <Stack direction="row" spacing={1} style={{ alignItems: 'flex-start' }}>
          <Tooltip
            title="dbpal use monaco editor as build in editor"
            placement="top"
          >
            <Typography width={headerSize} textAlign="right">
              monaco editor options:
            </Typography>
          </Tooltip>
          <Stack style={{ flex: 1 }} spacing={1}>
            <Stack direction="row" style={{ alignItems: 'center' }}>
              <IconButton
                onClick={() => {
                  setShowEditorOptions((e) => !e);
                }}
                color={showEditorOptions ? 'success' : 'primary'}
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
                  border: '2px solid' + theme.palette.success.main,
                  borderRadius: '8px',
                  padding: '8px',
                  display: showEditorOptions ? 'flex' : 'none',
                  transform: 'all 0.7s ease',
                }}
                editorOptions={editorOptions}
                setEditorOptions={setEditorOptions}
              ></EditorOptions>
            )}
          </Stack>
        </Stack>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          editor theme:
        </Typography>
        <Select
          onChange={(e) => setCustomEditorTheme(e.target.value)}
          style={{ minWidth: '25rem' }}
          value={customEditorTheme}
        >
          <MenuItem value=""></MenuItem>
          {['vs', 'vs-dark', 'hc-black', 'hc-light'].map((theme) => (
            <MenuItem value={theme}>{theme}</MenuItem>
          ))}
          {registeredThemes.map((theme) => {
            return (
              <MenuItem value={theme} key={theme}>
                {theme}
              </MenuItem>
            );
          })}
        </Select>

        <Tooltip
          title={
            <EditorThemeDemo
              width="55vw"
              fontSize={fontSize}
              theme={customEditorTheme}
            ></EditorThemeDemo>
          }
          slotProps={{
            tooltip: {
              style: { maxWidth: '55vw' },
            },
          }}
        >
          <span>
            <InfoIcon> </InfoIcon>
          </span>
        </Tooltip>
      </Stack>
      <Divider textAlign="left" style={{ marginTop: '20px' }}>
        export
      </Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          csv delimiter:
        </Typography>
        <TextField
          value={csvDelimiter}
          onChange={(e) => setCsvDelimiter(e.target.value)}
          error={csvDelimiter.length > 1}
          helperText="only accept one character"
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          include header:
        </Typography>
        <FormControlLabel
          control={
            <Switch
              checked={includeHeader}
              onChange={(e) => setIncludeHeader(e.target.checked)}
            />
          }
          label="include header"
        />
      </Stack>
      <Divider textAlign="left" style={{ marginTop: '20px' }}>
        auto start
      </Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          auto start:
        </Typography>

        <Switch
          checked={autoStart}
          onChange={(e) => {
            setAutoStart(e.target.checked);
          }}
        ></Switch>
      </Stack>
      <Divider />
      <Stack direction="row">
        <Button
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

function EditorThemeDemo(props: { width: string; theme: string; fontSize }) {
  const { theme, width, fontSize } = props;
  return (
    <Stack style={{ width, height: '30vh' }} spacing={1}>
      <Editor
        value={`select * from employees where id is not null limit 10;`}
        width="99%"
        height="99%"
        language="sql"
        options={{ fontSize }}
        theme={theme}
      ></Editor>
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
      <SimpleFlexEditor
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
      ></SimpleFlexEditor>
      <Divider />
      <Stack direction="row" spacing={1}>
        <Button
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

function makeComponents(fontWeight) {
  const components: Components<
    Omit<Theme, 'components' | 'palette'> & CssVarsTheme
  > = {
    MuiCircularProgress: {
      defaultProps: { size: '1.1rem' },
    },
    MuiButton: {
      styleOverrides: {
        root: {
          fontSize: '1rem',
          textTransform: 'none',
          fontWeight,
        },
      },
      defaultProps: {
        variant: 'contained',
        size: 'small',
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
          fontWeight,
        },
      },
    },
    MuiTooltip: {
      styleOverrides: {
        tooltip: {
          fontSize: '0.9rem',
        },
      },
      defaultProps: { arrow: true },
    },
    MuiAlertTitle: {
      styleOverrides: {
        root: {
          fontWeight,
        },
      },
    },
    //@ts-ignore
    MuiDataGrid: {
      styleOverrides: {
        root: {
          '& .MuiDataGrid-columnHeaderTitle': {
            fontWeight,
          },
        },
        cell: {
          fontSize: '1rem',
        },
        columnHeaders: {
          fontSize: '0.8rem',
          boxShadow: '0 1px 3px',
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
            fontWeight,
          },
        },
      },
      defaultProps: {
        size: 'small',
      },
    },
    MuiSelect: {
      defaultProps: { size: 'small' },
    },

    MuiChip: {
      styleOverrides: {
        root: {
          fontSize: '1rem',
          fontWeight,
        },
      },
      defaultProps: {
        size: 'small',
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

function SettingComp(props: { children: ReactNode }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSetting>(null);
  const reload = () => {
    UserSetting.get()
      .then((setting) => {
        setSetting(setting);
        document.documentElement.style.fontSize = setting.fontSize + 'px';
        document.documentElement.style.fontFamily = setting.fontFamily;
        document.documentElement.style.fontWeight = setting.fontWeight;
        if (setting.autoStart) {
          if (!isEnabledApp()) {
            enableApp().catch((e) => {
              console.log('enable app autostart failed', e);
              message(`enable app autostart failed ${e.toString()}`, {
                kind: 'error',
              });
            });
          }
        } else {
          if (isEnabledApp()) {
            disableApp().catch((e) => {
              console.log('disable app autostart failed', e);
              message('disable app autostart failed', { kind: 'error' });
            });
          }
        }
      })
      .catch((e) => {
        message(`list user settings failed ${e.toString()}`, { kind: 'error' });
        console.log('list user setting failed', e);
        setSetting(new UserSetting());
      });
  };
  useEffect(() => {
    const unlisten = appStore.onKeyChange<UserSetting>(
      AppStoreKey.UserSetting,
      () => {
        reload();
      }
    );
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);
  useEffect(() => {
    reload();
  }, []);
  return (
    setting && (
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
    )
  );
}

export { ModifySetting, SettingComp };
