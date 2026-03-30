import { ReactNode, useCallback, useEffect, useState } from "react";
import { FaFolderOpen } from "react-icons/fa";
import { isEqual } from "lodash-es";
import * as monaco from "monaco-editor";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
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
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import {
  validJson,
  registeredThemes,
  dataGridheaderFontSize,
  appStore,
  LLMConfig,
  shrinkAble,
  prettyMs,
  isDarkStr,
  MonacoEditor,
} from "./helper";

import { invoke } from "@tauri-apps/api/core";
import EditIcon from "@mui/icons-material/Edit";
import ClearIcon from "@mui/icons-material/Clear";
import { IconModifed, SimpleFlexEditor, StackRowCenter } from "./CommonComp2";
import {
  enable as enableApp,
  isEnabled as isEnabledApp,
  disable as disableApp,
} from "@tauri-apps/plugin-autostart";
import { message } from "@tauri-apps/plugin-dialog";
import { EscDownEvent, modalHandleClose, modalStyle } from "./Common";
import { LogLevelSelection, ModalHeader } from "./CommonComp";
import { appLogDir } from "@tauri-apps/api/path";
import {
  makeLLMCallable,
  useDatagridHeight,
  useLLMConfig,
  useNotification,
  UserSettings,
  useUserSettings,
} from "./store";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

function AppearanceSetting() {
  const { appearance, updateAppearance } = useUserSettings((s) => s);
  const { showNotification } = useNotification((s) => s);
  const [allFonts, setAllFonts] = useState<string[]>([]);
  const [fontWeight, setFontWeight] = useState(appearance.fontWeight);
  const [fontSize, setFontSize] = useState(appearance.fontSize);
  const [darkBgColor, setDarkBgColor] = useState(appearance.darkBgColor);
  const [lightBgColor, setLightBgColor] = useState(appearance.lightBgColor);
  const [fontFamily, setFontFamily] = useState(appearance.fontWeight);
  const [datagridRowHeightFactor, setDatagridRowHeightFactor] = useState(
    appearance.datagridRowHeightFactor + "",
  );
  const [intputVariant, setInputVariant] = useState(appearance.intputVariant);
  const [modified, setModified] = useState(false);

  const [theme, setTheme] = useState(appearance.theme);

  const [zoom, setZoom] = useState(1);

  useEffect(() => {
    const key = "zoom";
    appStore.get<number>(key).then((z) => {
      setZoom(z);
    });
    const unlisten = appStore.onKeyChange<number>(key, (z) => {
      setZoom(z);
    });
    return () => {
      unlisten.then((f) => {
        f();
      });
    };
  }, []);

  useEffect(() => {
    invoke<string[]>("all_font_family", {})
      .then((fonts) => {
        setAllFonts(fonts);
      })
      .catch((e) => {
        showNotification("error", "list all fonts failed", e);
      });
  }, [showNotification]);

  const makeConfig = useCallback(() => {
    const x = {
      fontFamily,
      fontSize,
      fontWeight,
      darkBgColor: darkBgColor,
      lightBgColor: lightBgColor,
      datagridRowHeightFactor: parseFloat(datagridRowHeightFactor),
      intputVariant,
      theme,
    };

    return x;
  }, [
    darkBgColor,
    datagridRowHeightFactor,
    fontFamily,
    fontSize,
    fontWeight,
    intputVariant,
    lightBgColor,
    theme,
  ]);

  useEffect(() => {
    setModified(!isEqual(appearance, makeConfig()));
  }, [makeConfig, appearance]);

  const handleSave = () => {
    try {
      updateAppearance(makeConfig());
    } catch (e) {
      showNotification("error", "save appearance setting failed", e);
      return;
    }
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
          onChange={(e) => {
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
                    edge="end">
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
          onChange={(e) => {
            setFontFamily(e.target.value);
          }}
          slotProps={{
            htmlInput: {
              list: "fonts_list",
            },
          }}
        />
        <datalist id="fonts_list">
          {allFonts.map((e) => (
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
          onChange={(e) => {
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
        <Typography width={headerSize}>theme </Typography>
        <Autocomplete
          options={["light", "dark", "system"]}
          style={{ width: "12rem" }}
          value={theme}
          renderInput={(params) => <TextField {...params} label={`theme`} />}
          freeSolo={false}
          onChange={(_, name) => {
            setTheme(name);
          }}
        />
      </Stack>
      <SetBgColor theme="dark" bgColor={darkBgColor} setBgColor={setDarkBgColor} />
      <SetBgColor theme="light" bgColor={lightBgColor} setBgColor={setLightBgColor} />

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>input variant</Typography>
        <Autocomplete
          options={["standard", "filled", "outlined"]}
          style={{ width: "22rem" }}
          value={intputVariant}
          renderInput={(params) => <TextField {...params} label="input variant" />}
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
        <Button onClick={handleSave} disabled={!modified}>
          {modified ? <IconModifed /> : undefined} save
        </Button>
      </Stack>
    </Stack>
  );
}

function SetBgColor({ bgColor, setBgColor, theme }) {
  const [showEditBgColor, setShowEditBgColor] = useState(false);
  return (
    <Stack direction="row" sx={{ alignItems: "center" }}>
      <Typography width={headerSize}> {theme} background color </Typography>
      <div style={{ position: "relative" }}>
        <IconButton
          color={showEditBgColor ? "success" : "primary"}
          onClick={() => {
            setShowEditBgColor((e) => !e);
          }}>
          <EditIcon> </EditIcon>
        </IconButton>
        {showEditBgColor && (
          <HexColorPicker
            color={bgColor}
            onChange={(newValue) => {
              setBgColor(newValue);
            }}
            style={{ position: "absolute", zIndex: 10, top: "3rem" }}
          />
        )}
      </div>

      <TextField
        value={bgColor}
        onChange={(e) => {
          setBgColor(e.target.value);
        }}
        style={{ width: "10rem" }}
        slotProps={{
          input: {
            endAdornment: bgColor && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setBgColor("");
                  }}
                  edge="end">
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      />
    </Stack>
  );
}

function EditorSetting() {
  const { editor, updateEditor } = useUserSettings((s) => s);
  const [showEditOption, setShowEditOption] = useState(false);
  const [tabSize, setTabSize] = useState(editor.tabSize);
  const [customEditorTheme, setCustomEditorTheme] = useState(editor.theme);
  const [highLightTheme, setHighlightTheme] = useState("");
  const { showNotification } = useNotification((s) => s);
  const [modified, setModified] = useState(false);

  const makeConfig = useCallback(() => {
    const obj = {
      tabSize,
      theme: customEditorTheme,
      editorOption: editor.editorOption,
    };
    return obj;
  }, [customEditorTheme, editor.editorOption, tabSize]);

  const handleSave = () => {
    try {
      updateEditor(makeConfig());
    } catch (e) {
      showNotification("error", "save  editor setting failed", e);
      return;
    }
  };

  useEffect(() => {
    setModified(!isEqual(editor, makeConfig()));
  }, [makeConfig, editor]);

  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>tab size </Typography>

        {[2, 4, 8].map((e) => (
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
                  setShowEditOption((e) => !e);
                }}
                color={showEditOption ? "success" : "primary"}>
                <EditIcon> </EditIcon>
              </IconButton>
            </StackRowCenter>
            {showEditOption && (
              <EditorOptions
                handleClose={() => {
                  setShowEditOption(false);
                }}
                showEditOption={showEditOption}
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
            renderInput={(params) => <TextField {...params} label="editor theme" />}
            freeSolo={false}
            onChange={(_, theme) => {
              setCustomEditorTheme(theme);
            }}
            onHighlightChange={(_, theme) => {
              setHighlightTheme(theme);
            }}
          />
          <EditorThemeDemo
            theme={highLightTheme ? highLightTheme : customEditorTheme}
          />
        </Stack>
      </Stack>
      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave} disabled={!modified}>
          {modified ? <IconModifed /> : undefined} save
        </Button>
      </Stack>
    </Stack>
  );
}

const headerSize = "16rem";

function ExportSetting() {
  const { export_, updateExport } = useUserSettings((s) => s);

  const [csvDelimiter, setCsvDelimiter] = useState(export_.csvDelimiter);
  const [includeHeader, setIncludeHeader] = useState(export_.includeHeader);
  const { showNotification } = useNotification((s) => s);
  const [modified, setModified] = useState(false);

  const makeConfig = useCallback(() => {
    const x = { csvDelimiter, includeHeader };

    return x;
  }, [csvDelimiter, includeHeader]);

  useEffect(() => {
    setModified(!isEqual(export_, makeConfig()));
  }, [makeConfig, export_]);

  const handleSave = () => {
    try {
      updateExport(makeConfig());
    } catch (e) {
      showNotification("error", "save export setting failed", e);
      return;
    }
  };

  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>csv delimiter</Typography>
        <TextField
          value={csvDelimiter}
          onChange={(e) => setCsvDelimiter(e.target.value)}
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
              onChange={(e) => setIncludeHeader(e.target.checked)}
            />
          }
          label="include header"
        />
      </Stack>

      <Divider />
      <Stack direction="row">
        <Button onClick={handleSave} disabled={!modified}>
          {modified ? <IconModifed /> : undefined} save
        </Button>
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
  const { config: oldSetting, setConfig } = useLLMConfig((s) => s);
  const { handleClose } = props;
  const [baseUrl, setBaseUrl] = useState(oldSetting.baseUrl);
  const [temperature, setTemperature] = useState(oldSetting.temperature + "");
  const [model, setModel] = useState(oldSetting.model);
  const [apiKey, setApiKey] = useState(oldSetting.apiKey);
  const { showNotification } = useNotification((s) => s);
  const [maxToken, setMaxToken] = useState(oldSetting.maxToken + "");
  const [topP, setTopP] = useState(oldSetting.topP + "");
  const [moddelCan, setModdelCan] = useState<ModelDef[]>([]);
  const [helloTime, setHelloTime] = useState(0);
  const [modified, setModified] = useState(false);

  const makeConfig = useCallback(() => {
    const config = {
      baseUrl,
      apiKey,
      temperature: parseFloat(temperature),
      model,
      maxToken: maxToken ? parseInt(maxToken) : null,
      topP: topP ? parseFloat(topP) : 0,
    } as LLMConfig;
    Object.setPrototypeOf(config, LLMConfig.prototype);
    return config;
  }, [apiKey, baseUrl, maxToken, model, temperature, topP]);

  useEffect(() => {
    setModified(!isEqual(oldSetting, makeConfig()));
  }, [makeConfig, oldSetting]);

  const handleSave = async () => {
    try {
      setConfig(makeConfig());
      if (handleClose) {
        handleClose();
      }
    } catch (e) {
      showNotification("error", "save llm config  failed", e);
      return;
    }
  };

  const handleHello = async () => {
    const now = new Date();
    const int = setInterval(() => {
      setHelloTime(new Date().getTime() - now.getTime());
    }, 400);
    try {
      const config = makeConfig();
      const resp = await makeLLMCallable(config).invoke("hello!");
      showNotification(
        "success",
        "config setting is ok,llm response:",
        <Typography color="info"> {resp} </Typography>,
      );
    } catch (e) {
      showNotification("error", "test llm config  failed", e);
      console.error("hello error", e);
    } finally {
      clearInterval(int);
      setTimeout(() => {
        setHelloTime(0);
      }, 1000);
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
      const ms = await listLLMModels(baseUrl, apiKey);
      setModdelCan(ms);
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
          onChange={(e) => setBaseUrl(e.target.value)}
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
                    edge="end">
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
          onChange={(e) => setApiKey(e.target.value)}
          type="password"
          slotProps={{
            input: {
              endAdornment: apiKey && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setApiKey("");
                    }}
                    edge="end">
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
          renderInput={(params) => (
            <TextField
              {...params}
              label={`model`}
              value={Modal}
              onChange={(e) => {
                setModel(e.target.validationMessage);
              }}
            />
          )}
          getOptionLabel={(e) => {
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
                }}>
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
          onChange={(e) => setTemperature(e.target.value)}
        />
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}> max token </Typography>
        <TextField
          error={maxToken ? isNaN(parseInt(maxToken)) : false}
          value={maxToken}
          onChange={(e) => setMaxToken(e.target.value)}
          slotProps={{
            input: {
              endAdornment: maxToken && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setMaxToken("");
                    }}
                    edge="end">
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
          onChange={(e) => setTopP(e.target.value)}
          slotProps={{
            input: {
              endAdornment: topP && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setTopP("");
                    }}
                    edge="end">
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
        <Button onClick={handleSave} disabled={!modified}>
          {modified ? <IconModifed /> : undefined} save
        </Button>

        <Button onClick={handleHello} variant="outlined" color="secondary">
          {helloTime ? (
            <>
              {prettyMs(helloTime)} <CircularProgress />
            </>
          ) : (
            "hello!"
          )}
        </Button>
      </Stack>
    </Stack>
  );
}

function MiscSetting() {
  const { misc, updateMisc } = useUserSettings((s) => s);

  console.log(misc);
  const [autoStart, setAutoStart] = useState(misc.autoStart);
  const [logLevel, setLogLevel] = useState(misc.logLevel);
  const { showNotification } = useNotification((s) => s);

  useEffect(() => {
    const x = { autoStart, logLevel };
    if (!isEqual(x, misc)) {
      console.log("save auto start", autoStart);
      try {
        updateMisc(x);
        showNotification("success", "save setting ok");
      } catch (e) {
        showNotification("error", "save setting  failed", e);
      }
    }
  }, [autoStart, logLevel, misc, showNotification, updateMisc]);

  return (
    <Stack style={{ textAlign: "right" }}>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>auto start</Typography>
        <Checkbox
          checked={autoStart}
          onChange={(e) => {
            setAutoStart(e.target.checked);
          }}
        />
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography width={headerSize}>logger level</Typography>
        <LogLevelSelection
          level={logLevel}
          handleChange={(level) => {
            setLogLevel(level);
            console.log("log level:", level);
          }}
        />
        {logLevel !== "none" && (
          <Tooltip title="open log directory">
            <IconButton
              onClick={async () => {
                openPath(await appLogDir());
              }}>
              <FaFolderOpen fontSize="1.5rem" />
            </IconButton>
          </Tooltip>
        )}
      </Stack>
    </Stack>
  );
}

function ModifySetting() {
  const ss = ["appearance", "editor", "export", "llm", "misc"];
  const [tab, setCurrentTab] = useState(ss[0]);
  return (
    <Stack>
      <Tabs
        value={tab}
        onChange={(_, value) => {
          setCurrentTab(value);
        }}>
        {ss.map((e) => (
          <Tab value={e} label={e} key={e} />
        ))}
      </Tabs>
      <div style={{ flex: 1 }}>
        {ss.map((setting) => {
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

function EditorThemeDemo(props: { theme: string }) {
  const { theme } = props;
  const [editor, setEditor] = useState<monaco.editor.IStandaloneCodeEditor>(null);

  useEffect(() => {
    if (!editor) {
      return;
    }
    if (!theme) {
      return;
    }
    // TODO not change
  }, [editor, theme]);

  return (
    <Stack
      style={{
        flexGrow: 1,
        flexShrink: 1,
        width: "100px",
        height: "15vh",
      }}>
      <MonacoEditor
        value={`select * from employees where id is not null limit 10;`}
        language="sql"
        readOnly={true}
        onMount={(ed) => {
          setEditor(ed);
        }}
      />
    </Stack>
  );
}
function EditorOptions(props: { handleClose; showEditOption }) {
  const { showNotification } = useNotification((s) => s);
  const { handleClose, showEditOption } = props;
  const { editor: editorOP, updateEditor: updateEditorOP } = useUserSettings((s) => s);
  const [value, setValue] = useState<string>(editorOP.editorOption);
  const muiTheme = useTheme();
  return (
    <Stack
      style={{
        height: "40vh",
        border: "2px solid" + muiTheme.palette.success.main,
        borderRadius: "8px",
        padding: "8px",
        display: showEditOption ? "flex" : "none",
        transform: "all 0.7s ease",
      }}>
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
        language="json"
        value={editorOP.editorOption}
        handleValue={(newValue) => {
          console.log("!!!!!!!!!!!!!!!", newValue);
          setValue(newValue);
        }}
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
            updateEditorOP({ ...editorOP, editorOption: value });
            handleClose();
          }}>
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
      defaultProps: { spacing: 0.7 },
    },
    MuiPagination: {
      defaultProps: {
        size: "small",
        showFirstButton: true,
        showLastButton: true,
      },
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

const makeTheme = (
  s: UserSettings,
  isDarkTheme: boolean,
  datagridRowHeightXXX: number,
) => {
  const { appearance } = s;
  let color: string;
  if (isDarkTheme) {
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
      mode: isDarkTheme ? "dark" : "light",
      background: {
        default: color,
        paper: color,
      },
    },
    components: makeComponents(
      fontWeight,
      appearance.intputVariant,
      datagridRowHeightXXX,
    ),
  });
};

const isDark = (setting: UserSettings, osTheme) => {
  return (
    isDarkStr(setting.appearance.theme) ||
    (setting.appearance.theme == "system" && isDarkStr(osTheme))
  );
};
function SettingProvider({ children }) {
  const setting = useUserSettings((s) => s);
  const [osTheme, setOsTheme] = useState("light");

  const { showNotification } = useNotification((s) => s);
  useEffect(() => {
    document.documentElement.style.fontSize = setting.appearance.fontSize + "px";
    document.documentElement.style.fontFamily = setting.appearance.fontFamily;
    document.documentElement.style.fontWeight = setting.appearance.fontWeight;
    if (setting.editor.theme) {
      monaco.editor.setTheme(setting.editor.theme);
    } else {
      monaco.editor.setTheme(isDark(setting, osTheme) ? "vs-dark" : "vs");
    }
    (async () => {
      if (setting.misc.autoStart) {
        if (!(await isEnabledApp())) {
          enableApp().catch((e) => {
            console.error("enable app autostart failed", e);
            message(`enable app autostart failed ${e.toString()}`, {
              kind: "error",
            });
          });
        }
      } else {
        if (await isEnabledApp()) {
          disableApp().catch((e) => {
            console.error("disable app autostart failed", e);
          });
        }
      }
      invoke("set_log_level", { level: setting.misc.logLevel }).catch((e) => {
        showNotification("error", "set log level failed", e);
      });
    })();
  }, [osTheme, setting, showNotification]);

  useEffect(() => {
    const currentWebview = WebviewWindow.getCurrent();
    currentWebview.theme().then(async (theme) => {
      if (!theme) {
        return;
      }
      setOsTheme(theme);
    });
    currentWebview.onThemeChanged(async (event) => {
      setOsTheme(event.payload);
    });
  }, []);

  const datagridHeightXXX = useDatagridHeight();

  return (
    setting && (
      <ThemeProvider
        theme={makeTheme(setting, isDark(setting, osTheme), datagridHeightXXX)}>
        <CssBaseline />
        {children}
      </ThemeProvider>
    )
  );
}

function ModifyLLMConfigModal() {
  const [showEdit, setShowEdit] = useState(false);
  const config = useLLMConfig((s) => s.config);
  return (
    <>
      {!config && (
        <Tooltip title="enable ai powered chat,need set up LLM server.">
          <Button
            onClick={() => {
              setShowEdit(true);
            }}
            style={shrinkAble}
            variant="outlined">
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
          })}>
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

export { ModifySetting, SettingProvider, ModifyLLMConfigModal, EditorSetting };
