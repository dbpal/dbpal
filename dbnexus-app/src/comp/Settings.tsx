import { CSSProperties, useContext, useEffect, useRef, useState } from 'react';

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
} from './helper';
import { Position } from 'monaco-editor';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import ClearIcon from '@mui/icons-material/Clear';
import { SimpleFlexEditor } from './CommonComp2';
import { LogLevelSelection } from './CommonComp';
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
  const [loggerLevel, setLoggerLevel] = useState('info');
  const [loggerMaxSize, setLoggerMaxSize] = useState('1');
  const [datagridRowHeightFactor, setDatagridRowHeightFactor] = useState('2');
  const [maxTreeSize, setMaxTreeSize] = useState('50');
  const [saveQueryLog, setSaveQueryLog] = useState(true);
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
      const setting = await UserSetting.get();
      setFontWeight(setting.fontWeight);
      setFontSize(setting.fontSize);
      setFontFamily(setting.fontFamily);
      setTabSize(setting.tabSize);
      if (setting.isDark()) {
        setBgColor(setting.darkBgColor);
      } else {
        setBgColor(setting.lightBgColor);
      }
      setLoggerLevel(setting.loggerLevel);
      setLoggerMaxSize(setting.loggerMaxSize + '');
      setCsvDelimiter(setting.csvDelimiter);
      setIncludeHeader(setting.includeHeader);
      setMaxTreeSize(setting.maxTreeSize.toString());
      setDatagridRowHeightFactor(setting.datagridRowHeightFactor.toString());
      setSaveQueryLog(setting.saveQueryLog);
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
    let s = {
      fontWeight,
      fontSize,
      fontFamily,
      tabSize,
      editorOption: JSON.parse(editorOptions),
      csvDelimiter: csvDelimiter,
      includeHeader: includeHeader,
      loggerLevel: loggerLevel,
      loggerMaxSize: parseInt(loggerMaxSize),
      datagridRowHeightFactor: parseFloat(datagridRowHeightFactor),
      maxTreeSize: parseInt(maxTreeSize),
      saveQueryLog: saveQueryLog,
      theme,
    } as UserSetting;
    if (theme == 'dark') {
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
      (theme == 'dark' && ColorFn.isLightHex(bgColor)) ||
      (theme == 'light' && !ColorFn.isLightHex(bgColor))
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
      <Divider textAlign="left">appearance</Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          font size:
        </Typography>
        <Chip label={fontSize} style={{ fontSize }} />
        <Slider
          style={{ width: '30%' }}
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
        <Typography
          width={headerSize}
          style={{ fontFamily: fontFamily }}
          textAlign="right"
        >
          font family:
        </Typography>

        <Select
          value={fontFamily}
          onChange={(e) => setFontFamily(e.target.value as string)}
          style={{ fontFamily: fontFamily, minWidth: '12rem' }}
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
                <MenuItem style={{ fontFamily: f }} value={f} key={f}>
                  {f}
                </MenuItem>
              );
            })}
        </Select>
        <TextField
          label="match font"
          value={fontMatch}
          style={{ fontFamily: fontFamily, width: '13rem' }}
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
          style={{ fontFamily, ...textEllipsisStyle }}
        >
          select * from employees
        </Typography>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography
          width={headerSize}
          style={{ fontFamily: fontFamily }}
          textAlign="right"
        >
          table row height factor:
        </Typography>
        <TextField
          value={datagridRowHeightFactor}
          onChange={(e) => {
            setDatagridRowHeightFactor(e.target.value);
          }}
          error={isNaN(parseFloat(datagridRowHeightFactor))}
          helperText="a float value,this parameter multiply font size as the table row height"
        ></TextField>
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
        <Typography
          width={headerSize}
          style={{ fontFamily: fontFamily }}
          textAlign="right"
        >
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
            color={e == tabSize ? 'primary' : 'default'}
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
                  border: '1px solid',
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
        logger
      </Divider>

      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          logger level:
        </Typography>

        <LogLevelSelection
          level={loggerLevel}
          handleChange={(level) => {
            setLoggerLevel(level);
          }}
        ></LogLevelSelection>
        <TextField
          value={loggerMaxSize}
          label="max logger file size (MB)"
          error={isNaN(parseInt(loggerMaxSize))}
          onChange={(e) => {
            setLoggerMaxSize(e.target.value);
          }}
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }} spacing={1}>
        <Typography width={headerSize} textAlign="right">
          auto save query log:
        </Typography>

        <Switch
          checked={saveQueryLog}
          onChange={(e) => {
            setSaveQueryLog(e.target.checked);
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
          padding: '2px  4px',
          textTransform: 'none',
          fontWeight,
        },
      },
      defaultProps: {
        variant: 'contained',
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

function SettingComp(props: { children }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSetting>(new UserSetting());
  const reload = async () => {
    UserSetting.get()
      .then((setting) => {
        setSetting(setting);
        document.documentElement.style.fontSize = setting.fontSize + 'px';
      })
      .catch((e) => {
        alert(`list user settings failed ${e.toString()}`);
        console.log('list user setting failed', e);
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

export { ModifySetting, SettingComp };
