import { CSSProperties, ReactNode, useContext, useEffect, useState } from "react";

import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  Components,
  createTheme,
  CssBaseline,
  CssVarsTheme,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  Modal,
  Slider,
  Stack,
  Tab,
  Tabs,
  TextField,
  Theme,
  ThemeProvider,
  Tooltip,
  Typography,
  useTheme,
} from "@mui/material";
import { HexColorPicker } from "react-colorful";
import { NotificationContext } from "./Notification";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
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
  appStore,
  LLMConfig,
  LLMConfigContext,
  shrinkAble,
  LoggerLevel,
} from "./helper";
import { Position } from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import EditIcon from "@mui/icons-material/Edit";
import ClearIcon from "@mui/icons-material/Clear";
import { SimpleFlexEditor, StackRowCenter } from "./CommonComp2";
import {
  enable as enableApp,
  isEnabled as isEnabledApp,
  disable as disableApp,
} from "@tauri-apps/plugin-autostart";
import { message } from "@tauri-apps/plugin-dialog";
import { Editor } from "@monaco-editor/react";
import { listen } from "@tauri-apps/api/event";
import { EscDownEvent, modalHandleClose, modalStyle } from "./Common";
import { LogLevelSelection, ModalHeader } from "./CommonComp";
import { appLogDir } from "@tauri-apps/api/path";

function AppearanceSetting() {
  const setting = UserSettingAppearance.get();
  const { showNotification } = useContext(NotificationContext);
  const [allFonts, setAllFonts] = useState<string[]>([]);
  const [showEditBgColor, setShowEditBgColor] = useState(false);
  const [fontWeight, setFontWeight] = useState(setting.fontWeight);
  const [fontSize, setFontSize] = useState(setting.fontSize);
  const [bgColor, setBgColor] = useState(
    isDarkTheme() ? setting.darkBgColor : setting.lightBgColor,
  );
  const [bgColorErr, setBgColorErr] = useState("");
  const [fontFamily, setFontFamily] = useState(setting.fontFamily);
  const [datagridRowHeightFactor, setDatagridRowHeightFactor] = useState(
    setting.datagridRowHeightFactor + "",
  );
  const [intputVariant, setInputVariant] = useState(setting.intputVariant);
  const [zoom, setZoom] = useState(1);
  useEffect(() => {
    if (!bgColor) {
      setBgColorErr("");
      return;
    }
    if (!ColorFn.isColor(bgColor)) {
      setBgColorErr("not a valid color");
      return;
    }
    if (
      (isDarkTheme() && ColorFn.isLightHex(bgColor)) ||
      (!isDarkTheme() && !ColorFn.isLightHex(bgColor))
    ) {
      setBgColorErr(`please set a '${appTheme}' background color`);
      return;
    }

    setBgColorErr("");
  }, [bgColor]);

  // useEffect(() => {
  //   const key = "zoom";
  //   appStore.get<number>(key).then(z => {
  //     setZoom(z);
  //   });
  //   const unlisten = appStore.onKeyChange<number>(key, z => {
  //     setZoom(z);
  //   });
  //   return () => {
  //     unlisten.then(f => {
  //       f();
  //     });
  //   };
  // }, []);

  useEffect(() => {
    invoke<string[]>("all_font_family", {})
      .then(fonts => {
        setAllFonts(fonts);
      })
      .catch(e => {
        showNotification("error", "list all fonts failed", e);
      });
  }, [showNotification]);

  const handleSave = () => {
    try {
      const x = {
        fontFamily,
        fontSize,
        fontWeight,
        darkBgColor: isDarkTheme() ? bgColor : setting.darkBgColor,
        lightBgColor: !isDarkTheme() ? bgColor : setting.lightBgColor,
        datagridRowHeightFactor: parseFloat(datagridRowHeightFactor),
        intputVariant,
      } as UserSettingAppearance;
      Object.setPrototypeOf(x, UserSettingAppearance.prototype);
      x.save();
    } catch (e) {
      showNotification("error", "save setting failed", e);
      return;
    }
    showNotification("success", "save setting success");
  };
  return (
    <Stack style={{ height: "100%", textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize} style={{ fontSize }}>
          font size
        </Typography>
        <Chip label={fontSize} style={{ fontSize }} />
        <Slider
          style={{ width: "30%" }}
          value={fontSize}
          onChange={(_, value) => {
            setFontSize(value as number);
          }}
          min={10}
          max={30}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize} style={{ fontWeight }}>
          font weight
        </Typography>
        <TextField
          value={fontWeight}
          onChange={e => {
            setFontWeight(e.target.value);
          }}
          slotProps={{
            input: {
              endAdornment: fontWeight && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFontWeight("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "flex-end" }}>
        <Typography width={headerSize} style={{ fontFamily }}>
          font
        </Typography>
        <TextField
          value={fontFamily}
          onChange={e => {
            setFontFamily(e.target.value);
          }}
          slotProps={{
            htmlInput: {
              list: "fonts_list",
            },
          }}
        />
        <datalist id="fonts_list">
          {allFonts.map(e => (
            <option key={e} value={e} />
          ))}
        </datalist>

        <Typography color="info" fontFamily={fontFamily}>
          SELECT * FROM employees WHERE gender = &apos;M&apos; LIMIT 100
        </Typography>
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>table row height factor </Typography>
        <TextField
          value={datagridRowHeightFactor}
          onChange={e => {
            setDatagridRowHeightFactor(e.target.value);
          }}
          error={isNaN(parseFloat(datagridRowHeightFactor))}
          helperText=""
        />
        <Tooltip title="a float value,this parameter multiply font size as the table row height">
          <Typography> ?? </Typography>
        </Tooltip>
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>background color </Typography>
        <div style={{ position: "relative" }}>
          <IconButton
            color={showEditBgColor ? "success" : "primary"}
            onClick={() => {
              setShowEditBgColor(e => !e);
            }}
          >
            <EditIcon> </EditIcon>
          </IconButton>
          {showEditBgColor && (
            <HexColorPicker
              color={bgColor}
              onChange={newValue => {
                setBgColor(newValue);
              }}
              style={{ position: "absolute", zIndex: 10, top: "3rem" }}
            />
          )}
        </div>

        <TextField
          value={bgColor}
          onChange={e => {
            setBgColor(e.target.value);
          }}
          style={{ width: "10rem" }}
          error={!!bgColorErr}
          slotProps={{
            input: {
              endAdornment: bgColor && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setBgColor("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
        {bgColorErr && <Typography color="error"> {bgColorErr} </Typography>}
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>input variant</Typography>
        <Autocomplete
          options={["standard", "filled", "outlined"]}
          style={{ width: "22rem" }}
          value={intputVariant}
          renderInput={params => <TextField {...params} label="input variant" />}
          freeSolo={false}
          onChange={(_, variant) => {
            if (!variant) {
              return;
            }
            setInputVariant(variant);
          }}
        />
        <TextField value={"this is demo"} variant={intputVariant as "filled"} />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>zoom</Typography>
        <Tooltip title="current zoom level">
          <Typography color="info"> {zoom}</Typography>
        </Tooltip>
      </Stack>
      <Divider />
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Button onClick={handleSave}> save </Button>
      </Stack>
    </Stack>
  );
}

function EditorSetting() {
  const setting = UserSettingEditor.get();
  const [editorOptions, setEditorOptions] = useState(
    setting.editorOption ? JSON.stringify(setting.editorOption) : "{}",
  );
  const [showEditOption, setShowEditOption] = useState(false);
  const [tabSize, setTabSize] = useState(setting.tabSize);
  const [customEditorTheme, setCustomEditorTheme] = useState(setting.theme);
  const [highLightTheme, setHighlightTheme] = useState("");
  const muiTheme = useTheme();
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      const obj = {
        tabSize,
        theme: customEditorTheme,
        editorOption: editorOptions ? JSON.parse(editorOptions) : null,
      } as UserSettingEditor;
      Object.setPrototypeOf(obj, UserSettingEditor.prototype);
      obj.save();
    } catch (e) {
      showNotification("error", "save setting  failed", e);
      return;
    }
    showNotification("success", "save setting ok");
  };
  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>tab size </Typography>

        {[2, 4, 8].map(e => (
          <Chip
            label={e}
            color={e == tabSize ? "success" : "default"}
            onClick={() => {
              setTabSize(e);
            }}
            key={e}
          />
        ))}
      </Stack>
      <Stack>
        <Stack direction="row" style={{ alignItems: "flex-start" }}>
          <Tooltip title="dbpal use monaco editor as build in editor" placement="top">
            <Typography width={headerSize}>monaco editor options</Typography>
          </Tooltip>
          <Stack style={{ flex: 1 }}>
            <StackRowCenter>
              <IconButton
                onClick={() => {
                  setShowEditOption(e => !e);
                }}
                color={showEditOption ? "success" : "primary"}
              >
                <EditIcon> </EditIcon>
              </IconButton>
              <Typography color="info">{editorOptions}</Typography>
            </StackRowCenter>

            {editorOptions && (
              <EditorOptions
                handleClose={() => {
                  setShowEditOption(false);
                }}
                style={{
                  height: "40vh",
                  border: "2px solid" + muiTheme.palette.success.main,
                  borderRadius: "8px",
                  padding: "8px",
                  display: showEditOption ? "flex" : "none",
                  transform: "all 0.7s ease",
                }}
                editorOptions={editorOptions}
                setEditorOptions={setEditorOptions}
              />
            )}
          </Stack>
        </Stack>
      </Stack>

      <Stack direction="row" sx={{ alignItems: "flex-start" }}>
        <Typography width={headerSize}>editor theme </Typography>
        <Stack direction="row" style={{ flex: 1, alignItems: "flex-start" }}>
          <Autocomplete
            options={registeredThemes}
            style={{ width: "22rem" }}
            value={customEditorTheme}
            renderInput={params => <TextField {...params} label="editor theme" />}
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
              width: "100px",
              height: "15vh",
            }}
            theme={highLightTheme ? highLightTheme : customEditorTheme}
          />
        </Stack>
      </Stack>
      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}> save </Button>
      </Stack>
    </Stack>
  );
}

const headerSize = "16rem";

function ExportSetting() {
  const setting = UserSettingExport.get();
  const [csvDelimiter, setCsvDelimiter] = useState(setting.csvDelimiter);
  const [includeHeader, setIncludeHeader] = useState(setting.includeHeader);
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      const x = { csvDelimiter, includeHeader } as UserSettingExport;
      Object.setPrototypeOf(x, UserSettingExport.prototype);
      x.save();
    } catch (e) {
      showNotification("error", "save setting  failed", e);
      return;
    }
    showNotification("success", "save setting ok");
  };

  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>csv delimiter</Typography>
        <TextField
          value={csvDelimiter}
          onChange={e => setCsvDelimiter(e.target.value)}
          error={csvDelimiter.length > 1}
          helperText="only accept one character"
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>include header</Typography>
        <FormControlLabel
          control={
            <Checkbox
              checked={includeHeader}
              onChange={e => setIncludeHeader(e.target.checked)}
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

interface ModelDef {
  id: string;
  context_length?: number;
  created: number;
}

async function listLLMModels(baseURl: string, apiKey: string): Promise<ModelDef[]> {
  const res = await fetch(`${baseURl}/models`, {
    headers: { Authorization: `Bearer ${apiKey}` },
  });
  if (!res.ok) {
    // 404、401 等会进这里
    throw new Error(`HTTP ${res.status}`);
  }
  const resp = await res.json();
  if (import.meta.env.DEV) {
    console.log("models response:", resp);
  }
  // data.object === "list"
  // data.data 是一个数组，每个元素有 id, object, owned_by, context_length 等字段
  return resp.data as ModelDef[];
}

function LLMSetting(props: { handleClose?: () => void }) {
  const { handleClose } = props;
  const [baseUrl, setBaseUrl] = useState("");
  const [temperature, setTemperature] = useState("0");
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [maxToken, setMaxToken] = useState("");
  const [topP, setTopP] = useState("");
  const [moddelCan, setModdelCan] = useState<ModelDef[]>([]);

  useEffect(() => {
    LLMConfig.get().then(e => {
      if (e) {
        setBaseUrl(e.baseUrl);
        setTemperature(e.temperature + "");
        setModel(e.model);
        setApiKey(e.apiKey);
        if (e.maxToken) {
          setMaxToken(e.maxToken + "");
        }

        if (e.topP) {
          setTopP(e.topP + "");
        }
      }
    });
  }, []);
  const makeConfig = () => {
    const config = {
      baseUrl,
      apiKey,
      temperature: parseFloat(temperature),
      model,
      maxToken: maxToken ? parseInt(maxToken) : null,
    } as LLMConfig;
    Object.setPrototypeOf(config, LLMConfig.prototype);
    return config;
  };
  const handleSave = async () => {
    try {
      const config = makeConfig();
      await config.save();
      if (handleClose) {
        handleClose();
      }
    } catch (e) {
      showNotification("error", "save llm config  failed", e);
      return;
    }
    showNotification("success", "save llm config ok");
  };

  const handleHello = async () => {
    setLoading(true);
    try {
      const config = makeConfig();
      const llm = config.createChatAPI();
      const resp = await llm.invoke("hello!");
      showNotification(
        "success",
        "config setting is ok,llm response:",
        <Typography color="info"> {resp.content as string} </Typography>,
      );
    } catch (e) {
      showNotification("error", "test llm config  failed", e);
    } finally {
      setLoading(false);
    }
  };

  const loadModelCan = async () => {
    if (!baseUrl) {
      showNotification("warning", "base url is empty");
      return;
    }
    if (!apiKey) {
      showNotification("warning", "api key is empty");
      return;
    }
    try {
      const x = await listLLMModels(baseUrl, apiKey);
      setModdelCan(x);
      showNotification("success", "load model candidates ok");
    } catch (e) {
      showNotification("error", "load model can failed", e);
    }
  };

  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>base url</Typography>
        <TextField
          style={{ width: "25rem" }}
          value={baseUrl}
          onChange={e => setBaseUrl(e.target.value)}
          slotProps={{
            htmlInput: {
              type: "url",
            },
            input: {
              endAdornment: baseUrl && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setBaseUrl("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>api key</Typography>
        <TextField
          value={apiKey}
          style={{ width: "40rem" }}
          onChange={e => setApiKey(e.target.value)}
          type="password"
          slotProps={{
            input: {
              endAdornment: apiKey && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setApiKey("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>model </Typography>
        <Button variant="outlined" onClick={loadModelCan}>
          load candidates
        </Button>
        <Autocomplete
          style={{ flex: 1, maxWidth: "32rem" }}
          options={moddelCan}
          value={model as any}
          renderInput={params => (
            <TextField
              {...params}
              label={`model`}
              value={Modal}
              onChange={e => {
                setModel(e.target.validationMessage);
              }}
            />
          )}
          getOptionLabel={e => {
            return typeof e == "string" ? e : e.id;
          }}
          renderOption={(props, def) => (
            <li {...props} key={def.id}>
              <Stack
                direction="row"
                style={{
                  justifyContent: "space-between",
                  alignItems: "center",
                  width: "100%",
                }}
              >
                <span> {def.id} </span>
                {def.context_length && (
                  <Stack direction="row" divider=<Divider />>
                    <Tooltip title="context legnth">
                      <span> {def.context_length / 1024}k</span>
                    </Tooltip>
                  </Stack>
                )}
              </Stack>
            </li>
          )}
          freeSolo={false}
          onChange={(_, name) => {
            setModel(typeof name == "string" ? name : name.id);
          }}
        />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>temperature</Typography>
        <TextField
          helperText="only accept number"
          value={temperature}
          error={temperature ? isNaN(parseFloat(temperature)) : false}
          onChange={e => setTemperature(e.target.value)}
        />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}> max token </Typography>
        <TextField
          error={maxToken ? isNaN(parseInt(maxToken)) : false}
          value={maxToken}
          onChange={e => setMaxToken(e.target.value)}
          slotProps={{
            input: {
              endAdornment: maxToken && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setMaxToken("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}> topP </Typography>
        <TextField
          error={topP ? isNaN(parseFloat(topP)) : false}
          value={topP}
          onChange={e => setTopP(e.target.value)}
          slotProps={{
            input: {
              endAdornment: topP && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setTopP("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>

      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}>save</Button>

        <Button
          onClick={handleHello}
          loading={loading}
          variant="outlined"
          color="secondary"
        >
          hello!
        </Button>
      </Stack>
    </Stack>
  );
}

function MiscSetting() {
  const [autoStart, setAutoStart] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleSave = () => {
    try {
      const x = { autoStart } as UserSettingMisc;
      Object.setPrototypeOf(x, UserSettingMisc.prototype);
      x.save();
    } catch (e) {
      showNotification("error", "save setting  failed", e);
      return;
    }
    showNotification("success", "save setting ok");
  };
  const [loglevel, setLogLevel] = useState(LoggerLevel.level);
  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>auto start</Typography>

        <Checkbox
          checked={autoStart}
          onChange={e => {
            setAutoStart(e.target.checked);
          }}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>logger level</Typography>
        <LogLevelSelection
          level={loglevel}
          handleChange={level => {
            setLogLevel(level);
            LoggerLevel.set(level);
          }}
        />
        {loglevel !== "none" && (
          <Tooltip title="open log directory">
            <Button
              onClick={async () => {
                openPath(await appLogDir());
              }}
            >
              open
            </Button>
          </Tooltip>
        )}
      </Stack>

      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave}>save</Button>
      </Stack>
    </Stack>
  );
}

function ModifySetting() {
  const ss = ["appearance", "editor", "export", "llm", "misc"];
  const [tab, setTab] = useState(ss[0]);
  return (
    <Stack>
      <Tabs
        value={tab}
        onChange={(_, value) => {
          setTab(value);
        }}
      >
        {ss.map(e => (
          <Tab value={e} label={e} key={e} />
        ))}
      </Tabs>
      <div style={{ flex: 1 }}>
        {ss.map(setting => {
          let content: ReactNode;
          switch (setting) {
            case "appearance":
              content = <AppearanceSetting />;
              break;
            case "editor":
              content = <EditorSetting />;
              break;
            case "export":
              content = <ExportSetting />;
              break;
            case "misc":
              content = <MiscSetting />;
              break;
            case "llm":
              content = <LLMSetting />;
              break;
          }
          return (
            <div key={setting} style={{ display: setting === tab ? "block" : "none" }}>
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
      />
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
    validJson(value).then(e => {
      setJsonErr(e);
    });
  }, [value]);
  return (
    <Stack style={style}>
      <Stack direction="row">
        <Chip
          color="info"
          style={{ cursor: "pointer" }}
          label="options???"
          onClick={() => {
            openUrl(
              "https://microsoft.github.io/monaco-editor/typedoc/variables/editor.EditorOptions.html",
            );
          }}
        />
      </Stack>
      <SimpleFlexEditor
        errPos={jsonErrPos}
        language="json"
        value={value}
        handleValue={v => {
          setValue(v);
        }}
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              style={{ cursor: "pointer" }}
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
      />
      <Divider />
      <Stack direction="row">
        <Button
          onClick={async () => {
            try {
              await validJson(value);
            } catch (e) {
              showNotification("error", "not a valid json", e);
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

function makeComponents(
  fontWeight: string | number,
  intputVariant: string,
  dataGridRowHeight: number,
) {
  const components: Components<Omit<Theme, "components" | "palette"> & CssVarsTheme> = {
    MuiCircularProgress: {
      defaultProps: { size: "1.2rem" },
    },
    MuiButtonGroup: {
      defaultProps: { size: "small", variant: "contained" },
    },
    MuiButton: {
      styleOverrides: {
        root: {
          fontSize: "1rem",
          textTransform: "none",
          fontWeight,
        },
      },
      defaultProps: {
        variant: "contained",
        size: "small",
      },
    },
    MuiIconButton: {
      styleOverrides: {
        root: {
          fontSize: "1rem",
          padding: "2px 4px",
          textTransform: "none",
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
          fontSize: "0.9rem",
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
    MuiToolbar: {
      defaultProps: { variant: "dense" },
    },
    MuiSlider: {
      defaultProps: { size: "small" },
    },
    MuiDivider: {
      styleOverrides: {
        root: {
          borderBottomWidth: "3px", // 横向
        },
        vertical: {
          borderRightWidth: "3px", // 竖向
        },
      },
    },
    //@ts-ignore
    MuiDataGrid: {
      styleOverrides: {
        root: {
          "& .MuiDataGrid-columnHeaderTitle": {
            fontWeight,
          },
        },
        cell: {
          fontSize: "1rem",
        },
        columnHeaders: {
          fontSize: dataGridheaderFontSize,
          boxShadow: "0 1px 3px",
        },
      },
      defaultProps: {
        density: "compact",
        disableRowSelectionOnClick: true,
        showCellVerticalBorder: true,
        showColumnVerticalBorder: true,
        rowHeight: dataGridRowHeight,
        columnHeaderHeight: dataGridRowHeight,
        showToolbar: true,
      },
    },
    MuiStack: {
      defaultProps: { spacing: 0.5 },
    },
    MuiPagination: {
      defaultProps: { size: "small" },
    },
    MuiMenuList: {
      styleOverrides: {
        root: {
          padding: "0px",
        },
      },
      defaultProps: {
        dense: true,
      },
    },
    MuiMenuItem: {
      styleOverrides: {
        root: {
          height: "1.6rem",
          fontSize: "1rem",
        },
      },
    },
    MuiTextField: {
      styleOverrides: {
        root: {
          "& .MuiInputBase-input": {
            fontWeight,
          },
        },
      },
      defaultProps: {
        size: "small",
        variant: intputVariant as "filled",
      },
    },
    MuiSelect: {
      defaultProps: {
        size: "small",
        variant: intputVariant as "filled",
      },
    },

    MuiChip: {
      styleOverrides: {
        root: {
          fontSize: "1rem",
          fontWeight,
        },
      },
      defaultProps: {
        size: "small",
      },
    },
    MuiTab: {
      styleOverrides: {
        root: {
          // textTransform: "none",
        },
      },
    },
    MuiAutocomplete: {
      defaultProps: { size: "small" },
    },
  };
  return components;
}

const makeTheme = (s: UserSetting) => {
  const { appearance } = s;
  let color: string;
  if (isDarkTheme()) {
    color = appearance.darkBgColor ? appearance.darkBgColor : "#000";
  } else {
    color = appearance.lightBgColor ? appearance.lightBgColor : "#fff";
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
      mode: isDarkTheme() ? "dark" : "light",
      background: {
        default: color,
        paper: color,
      },
    },
    components: makeComponents(
      fontWeight,
      appearance.intputVariant,
      s.datagridRowHeightXXX(),
    ),
  });
};

function SettingComp(props: { children: ReactNode }) {
  const { children } = props;
  const [setting, setSetting] = useState<UserSetting>(null);
  const reload = async () => {
    const setting = new UserSetting();
    setSetting(setting);
    document.documentElement.style.fontSize = setting.appearance.fontSize + "px";
    document.documentElement.style.fontFamily = setting.appearance.fontFamily;
    document.documentElement.style.fontWeight = setting.appearance.fontWeight;
    if (setting.misc.autoStart) {
      if (!(await isEnabledApp())) {
        enableApp().catch(e => {
          console.error("enable app autostart failed", e);
          message(`enable app autostart failed ${e.toString()}`, {
            kind: "error",
          });
        });
      }
    } else {
      if (await isEnabledApp()) {
        disableApp().catch(e => {
          console.error("disable app autostart failed", e);
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
      unlisten.then(callback => {
        callback();
      });
    };
  }, []);

  return (
    setting && (
      <ThemeProvider theme={makeTheme(setting)}>
        <CssBaseline />
        <SettingContext.Provider value={setting}>{children}</SettingContext.Provider>
      </ThemeProvider>
    )
  );
}

function ModifyLLMConfigModal() {
  const [showEdit, setShowEdit] = useState(false);
  const config = useContext(LLMConfigContext);
  return (
    <>
      {!config && (
        <Tooltip title="enable ai powered chat,need set up LLM server.">
          <Button
            onClick={() => {
              setShowEdit(true);
            }}
            style={shrinkAble}
            variant="outlined"
          >
            ai chat?
          </Button>
        </Tooltip>
      )}

      {showEdit && (
        <Modal
          open={showEdit}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEdit(false);
          })}
        >
          <Box sx={modalStyle} width="65vw">
            <ModalHeader
              title="edit llm config"
              handleClose={() => {
                setShowEdit(false);
              }}
            />
            <LLMSetting
              handleClose={() => {
                setShowEdit(false);
              }}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

export { ModifySetting, SettingComp, ModifyLLMConfigModal };
