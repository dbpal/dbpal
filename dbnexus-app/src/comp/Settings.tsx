'use strict';
import {
  CSSProperties,
  ReactNode,
  useContext,
  useEffect,
  useState,
} from 'react';

import {
  Autocomplete,
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
  Slider,
  Stack,
  Switch,
  Tab,
  Tabs,
  TextField,
  Theme,
  ThemeProvider,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import { HexColorPicker } from 'react-colorful';
import { NotificationContext } from './Notification';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  ErrAndLoc,
  ColorFn,
  SettingContext,
  UserSetting,
  validJson,
  registeredThemes,
  UserSettingAppearance,
  UserSettingExport,
  UserSettingEditor,
  settingChangeEvent,
  UserSettingMisc,
  dataGridheaderFontSize,
  isDarkTheme,
  appTheme,
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
import { Editor } from '@monaco-editor/react';
import { listen } from '@tauri-apps/api/event';

function AppearanceSetting() {
  const setting = UserSettingAppearance.get();
  const { showNotification } = useContext(NotificationContext);
  const [allFonts, setAllFonts] = useState<string[]>([]);
  const [showEditBgColor, setShowEditBgColor] = useState(false);
  const [fontWeight, setFontWeight] = useState(setting.fontWeight);
  const [fontSize, setFontSize] = useState(setting.fontSize);
  const [bgColor, setBgColor] = useState(
    isDarkTheme() ? setting.darkBgColor : setting.lightBgColor
  );
  const [bgColorErr, setBgColorErr] = useState('');
  const [fontFamily, setFontFamily] = useState(setting.fontFamily);
  const [datagridRowHeightFactor, setDatagridRowHeightFactor] = useState(
    setting.datagridRowHeightFactor + ''
  );
  const [maxTreeSize, setMaxTreeSize] = useState(setting.maxTreeSize + '');
  const [maxTabTitleLength, setMaxTabTitleLength] = useState(
    setting.maxTabTitleLength + ''
  );
  const [intputVariant, setInputVariant] = useState(setting.intputVariant);

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
      (isDarkTheme() && ColorFn.isLightHex(bgColor)) ||
      (!isDarkTheme() && !ColorFn.isLightHex(bgColor))
    ) {
      setBgColorErr(`please set a '${appTheme}' background color`);
      return;
    }
    setBgColorErr('');
  }, [bgColor]);

  useEffect(() => {
    invoke<string[]>('all_font_family', {})
      .then((fonts) => {
        setAllFonts(fonts);
      })
      .catch((e) => {
        showNotification('error', 'list all fonts failed', e);
      });
  }, []);

  const handleSave = () => {
    try {
      let x = {
        fontFamily,
        fontSize,
        fontWeight,
        darkBgColor: isDarkTheme() ? bgColor : setting.darkBgColor,
        lightBgColor: !isDarkTheme() ? bgColor : setting.lightBgColor,
        datagridRowHeightFactor: parseFloat(datagridRowHeightFactor),
        maxTreeSize: parseInt(maxTreeSize),
        intputVariant,
        maxTabTitleLength: parseInt(maxTabTitleLength),
      } as UserSettingAppearance;
      Object.setPrototypeOf(x, UserSettingAppearance.prototype);
      x.save();
    } catch (e) {
      showNotification('error', 'save setting failed', e);
      return;
    }
    showNotification('success', 'save setting success');
  };
  return (
    <Stack style={{ height: '100%', textAlign: 'right' }}>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize} style={{ fontSize }}>
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
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize} style={{ fontWeight }}>
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
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize} style={{ fontFamily }}>
          font family:
        </Typography>
        <Autocomplete
          options={allFonts}
          style={{ flex: 1 }}
          value={fontFamily}
          renderInput={(params) => (
            <TextField {...params} label="font family" />
          )}
          renderOption={(props, option) => (
            <li {...props} style={{ fontFamily: option }}>
              <span style={{ width: '22rem' }}>{option} </span>
              SELECT * FROM employees WHERE gender = 'M' LIMIT 100
            </li>
          )}
          freeSolo={false}
          onChange={(_, f) => {
            setFontFamily(f);
          }}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>table row height factor:</Typography>
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
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>background color:</Typography>
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
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>max tree size:</Typography>
        <TextField
          value={maxTreeSize}
          onChange={(e) => {
            setMaxTreeSize(e.target.value);
          }}
          label="max tree item size"
          error={isNaN(parseInt(maxTreeSize))}
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>max tab title length:</Typography>
        <TextField
          value={maxTabTitleLength}
          onChange={(e) => {
            setMaxTabTitleLength(e.target.value);
          }}
          error={isNaN(parseInt(maxTabTitleLength))}
        ></TextField>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>input variant:</Typography>
        <Autocomplete
          options={['standard', 'filled', 'outlined']}
          style={{ width: '22rem' }}
          value={intputVariant}
          renderInput={(params) => (
            <TextField {...params} label="input variant" />
          )}
          freeSolo={false}
          onChange={(_, variant) => {
            if (!variant) {
              return;
            }
            setInputVariant(variant);
          }}
        />
        <TextField
          value={'this is demo'}
          variant={intputVariant as 'filled'}
        ></TextField>
      </Stack>
      <Divider></Divider>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Button onClick={handleSave}> save </Button>
      </Stack>
    </Stack>
  );
}

function EditorSetting() {
  const setting = UserSettingEditor.get();
  const [editorOptions, setEditorOptions] = useState(
    setting.editorOption ? JSON.stringify(setting.editorOption) : '{}'
  );
  const [showEditOption, setShowEditOption] = useState(false);
  const [tabSize, setTabSize] = useState(setting.tabSize);
  const [customEditorTheme, setCustomEditorTheme] = useState(setting.theme);
  const [highLightTheme, setHighlightTheme] = useState('');
  const muiTheme = useTheme();
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      let obj = {
        tabSize,
        theme: customEditorTheme,
        editorOption: editorOptions ? JSON.parse(editorOptions) : null,
      } as UserSettingEditor;
      Object.setPrototypeOf(obj, UserSettingEditor.prototype);
      obj.save();
    } catch (e) {
      showNotification('error', 'save setting  failed', e);
      return;
    }
    showNotification('success', 'save setting ok');
  };
  return (
    <Stack style={{ textAlign: 'right' }}>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>tab size:</Typography>

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
      <Stack>
        <Stack direction="row" style={{ alignItems: 'flex-start' }}>
          <Tooltip
            title="dbpal use monaco editor as build in editor"
            placement="top"
          >
            <Typography width={headerSize}>monaco editor options:</Typography>
          </Tooltip>
          <Stack style={{ flex: 1 }}>
            <Stack direction="row" style={{ alignItems: 'center' }}>
              <IconButton
                onClick={() => {
                  setShowEditOption((e) => !e);
                }}
                color={showEditOption ? 'success' : 'primary'}
              >
                <EditIcon> </EditIcon>
              </IconButton>
              <Typography color="info">{editorOptions}</Typography>
            </Stack>

            {editorOptions && (
              <EditorOptions
                handleClose={() => {
                  setShowEditOption(false);
                }}
                style={{
                  height: '40vh',
                  border: '2px solid' + muiTheme.palette.success.main,
                  borderRadius: '8px',
                  padding: '8px',
                  display: showEditOption ? 'flex' : 'none',
                  transform: 'all 0.7s ease',
                }}
                editorOptions={editorOptions}
                setEditorOptions={setEditorOptions}
              ></EditorOptions>
            )}
          </Stack>
        </Stack>
      </Stack>

      <Stack direction="row" sx={{ alignItems: 'flex-start' }}>
        <Typography width={headerSize}>editor theme:</Typography>
        <Stack direction="row" style={{ flex: 1, alignItems: 'flex-start' }}>
          <Autocomplete
            options={registeredThemes}
            style={{ width: '22rem' }}
            value={customEditorTheme}
            renderInput={(params) => (
              <TextField {...params} label="editor theme" />
            )}
            freeSolo={false}
            onChange={(_, theme) => {
              setCustomEditorTheme(theme);
            }}
            onHighlightChange={(_, theme) => {
              setHighlightTheme(theme);
            }}
          />
          <EditorThemeDemo
            style={{
              flexGrow: 1,
              flexShrink: 1,
              width: '100px',
              height: '15vh',
            }}
            theme={highLightTheme ? highLightTheme : customEditorTheme}
          ></EditorThemeDemo>
        </Stack>
      </Stack>
      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}> save </Button>
      </Stack>
    </Stack>
  );
}

const headerSize = '16rem';

function ExportSetting() {
  const setting = UserSettingExport.get();
  const [csvDelimiter, setCsvDelimiter] = useState(setting.csvDelimiter);
  const [includeHeader, setIncludeHeader] = useState(setting.includeHeader);
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      let x = { csvDelimiter, includeHeader } as UserSettingExport;
      Object.setPrototypeOf(x, UserSettingExport.prototype);
      x.save();
    } catch (e) {
      showNotification('error', 'save setting  failed', e);
      return;
    }
    showNotification('success', 'save setting ok');
  };

  return (
    <Stack style={{ textAlign: 'right' }}>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>csv delimiter:</Typography>
        <TextField
          value={csvDelimiter}
          onChange={(e) => setCsvDelimiter(e.target.value)}
          error={csvDelimiter.length > 1}
          helperText="only accept one character"
        ></TextField>
      </Stack>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>include header:</Typography>
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

      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}>save</Button>
      </Stack>
    </Stack>
  );
}

function MiscSetting() {
  const [autoStart, setAutoStart] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      let x = { autoStart } as UserSettingMisc;
      Object.setPrototypeOf(x, UserSettingMisc.prototype);
      x.save();
    } catch (e) {
      showNotification('error', 'save setting  failed', e);
      return;
    }
    showNotification('success', 'save setting ok');
  };
  return (
    <Stack style={{ textAlign: 'right' }}>
      <Stack direction="row" sx={{ alignItems: 'center' }}>
        <Typography width={headerSize}>auto start:</Typography>

        <Switch
          checked={autoStart}
          onChange={(e) => {
            setAutoStart(e.target.checked);
          }}
        ></Switch>
      </Stack>
      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}>save</Button>
      </Stack>
    </Stack>
  );
}

function ModifySetting() {
  const ss = ['appearance', 'editor', 'export', 'misc'];
  const [tab, setTab] = useState(ss[0]);
  return (
    <Stack>
      <Tabs
        value={tab}
        onChange={(_, value) => {
          setTab(value);
        }}
      >
        {ss.map((e) => (
          <Tab value={e} label={e} key={e} />
        ))}
      </Tabs>
      <div style={{ flex: 1 }}>
        {ss.map((setting) => {
          let content: ReactNode;
          switch (setting) {
            case 'appearance':
              content = <AppearanceSetting></AppearanceSetting>;
              break;
            case 'editor':
              content = <EditorSetting></EditorSetting>;
              break;
            case 'export':
              content = <ExportSetting></ExportSetting>;
              break;
            case 'misc':
              content = <MiscSetting></MiscSetting>;
              break;
          }
          return (
            <div
              key={setting}
              style={{ display: setting === tab ? 'block' : 'none' }}
            >
              {content}
            </div>
          );
        })}
      </div>
    </Stack>
  );
}

function EditorThemeDemo(props: { style: CSSProperties; theme: string }) {
  const setting = useContext(SettingContext);
  const { fontWeight, fontSize, fontFamily } = setting.appearance;
  const { theme, style } = props;
  return (
    <Stack style={style}>
      <Editor
        value={`select * from employees where id is not null limit 10;`}
        width="99%"
        height="99%"
        language="sql"
        options={{ fontSize, fontFamily, fontWeight, readOnly: true }}
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
    validJson(value).then((e) => {
      setJsonErr(e);
    });
  }, [value]);
  return (
    <Stack style={style}>
      <Stack direction="row">
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
      <Stack direction="row">
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

function makeComponents(fontWeight: string | number, intputVariant: string) {
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
    MuiSlider: {
      defaultProps: { size: 'small' },
    },
    MuiDivider: {
      styleOverrides: {
        root: {
          borderBottomWidth: '3px', // 横向
        },
        vertical: {
          borderRightWidth: '3px', // 竖向
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
          fontSize: dataGridheaderFontSize,
          boxShadow: '0 1px 3px',
        },
      },
    },
    MuiStack: {
      defaultProps: { spacing: 0.5 },
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
        variant: intputVariant as 'filled',
      },
    },
    MuiSelect: {
      defaultProps: {
        size: 'small',
        variant: intputVariant as 'filled',
      },
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

const makeTheme = (appearance: UserSettingAppearance) => {
  let color;
  if (isDarkTheme()) {
    color = appearance.darkBgColor ? appearance.darkBgColor : '#000';
  } else {
    color = appearance.lightBgColor ? appearance.lightBgColor : '#fff';
  }
  const { fontWeight, fontFamily } = appearance;
  return createTheme({
    typography: {
      allVariants: {
        fontWeight,
      },
      fontFamily,
    },
    palette: {
      mode: isDarkTheme() ? 'dark' : 'light',
      background: {
        default: color,
        paper: color,
      },
    },
    components: makeComponents(fontWeight, appearance.intputVariant),
  });
};

function SettingComp(props: { children: ReactNode }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSetting>(null);
  const reload = async () => {
    let setting = new UserSetting();
    setSetting(setting);
    document.documentElement.style.fontSize =
      setting.appearance.fontSize + 'px';
    document.documentElement.style.fontFamily = setting.appearance.fontFamily;
    document.documentElement.style.fontWeight = setting.appearance.fontWeight;
    if (setting.misc.autoStart) {
      if (!(await isEnabledApp())) {
        enableApp().catch((e) => {
          console.error('enable app autostart failed', e);
          message(`enable app autostart failed ${e.toString()}`, {
            kind: 'error',
          });
        });
      }
    } else {
      if (await isEnabledApp()) {
        disableApp().catch((e) => {
          console.error('disable app autostart failed', e);
        });
      }
    }
  };
  useEffect(() => {
    reload();
  }, []);

  useEffect(() => {
    const unlisten = listen(settingChangeEvent, () => {
      reload();
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, []);

  return (
    setting && (
      <ThemeProvider theme={makeTheme(setting.appearance)}>
        <CssBaseline />
        <SettingContext.Provider value={setting}>
          {children}
        </SettingContext.Provider>
      </ThemeProvider>
    )
  );
}

export { ModifySetting, SettingComp };
