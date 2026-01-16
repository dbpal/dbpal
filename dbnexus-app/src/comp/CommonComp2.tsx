import {
  CSSProperties,
  ReactNode,
  RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ClearIcon from "@mui/icons-material/Clear";
import { NotificationContext } from "./Notification";
import Stack from "@mui/material/Stack";
import InfoIcon from "@mui/icons-material/Info";
import CloseIcon from "@mui/icons-material/Close";
import SaveIcon from "@mui/icons-material/Save";
import { FaBookmark } from "react-icons/fa";
import { IoMdInformationCircleOutline } from "react-icons/io";
import CancelIcon from "@mui/icons-material/Cancel";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  FormControlLabel,
  FormLabel,
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Modal,
  Pagination,
  Paper,
  Radio,
  RadioGroup,
  Slider,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AttachFileIcon from "@mui/icons-material/AttachFile";
import {
  DataSourceKind,
  editorPositionToEnd,
  editorSetPositionAndReveal,
  FavoriteSql,
  generateRandomString,
  getLanguageFromDataSourceKind,
  isDarkTheme,
  isMacOs,
  KeyPressed,
  quoteDatabaseIdentifier,
  saveAndSetSize,
  secondConfirm,
  SetStatusFn,
  SettingContext,
  TabActiveContext,
  TabContent,
  TabsSetContext,
} from "./helper";
import { Editor } from "@monaco-editor/react";
import { editor, Position } from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import {
  ConnectionHandler,
  EscDownEvent,
  extractBaseName,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  reloadOK,
  SetAutoCompletionName,
  TableFilter,
  TableFilterHandler,
} from "./Common";
import AddIcon from "@mui/icons-material/Add";
import types from "./ApiStruct";
import { MetaValueView, ModalHeader, SelectPageSize } from "./CommonComp";
import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { save, open, DialogFilter, message } from "@tauri-apps/plugin-dialog";
import ReactMarkdown from "react-markdown";
import { oneDark, oneLight } from "react-syntax-highlighter/dist/esm/styles/prism";
import { styled } from "@mui/material/styles";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  gridPageCountSelector,
  gridPaginationModelSelector,
  useGridApiContext,
  useGridSelector,
} from "@mui/x-data-grid";
import { CustomToolbar } from "./CustomToolBar";
function CreateDatabase({ uuid, handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const [database, setDatabase] = useState("");
  const databaseDom = useRef(null);
  const handleCreate = () => {
    if (!database) {
      databaseDom.current.focus();
      showNotification("warning", "database name is empty");
      return;
    }

    const extra = null;
    invoke("create_database", { name: uuid, database, extra })
      .then(() => {
        showNotification("success", "Create OK");
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Create Failure", e);
      });
  };

  return (
    <Stack>
      <Stack>
        <TextField
          inputRef={databaseDom}
          value={database}
          onChange={e => {
            setDatabase(e.target.value);
          }}
          label="database name"
        />
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          create
        </Button>
      </Stack>
    </Stack>
  );
}

function CreateSchema(props: {
  uuid;
  handleClose;
  handleReload;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { uuid, handleClose, kind, schemas, handleReload } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useContext(NotificationContext);
  const [schema, setSchema] = useState("");
  const schemaDom = useRef(null);
  const replicationStrategies = ["SimpleStrategy", "NetworkTopologyStrategy"];
  const [replicationStrategy, setReplicationStrategy] = useState(
    replicationStrategies[0],
  );
  const [replicationFactor, setReplicationFactor] = useState("3");
  const handleCreate = () => {
    if (!schema) {
      schemaDom.current.focus();
      showNotification("error", "Schema is empty");
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification("warning", `'${schema}' already exists.`);
      return;
    }
    let extra = null;
    switch (kind) {
      case DataSourceKind.Cassandra:
        extra = {
          extra: "cassandra",
          replication_strategy: replicationStrategy,
          replication_factor: parseInt(replicationFactor),
        };
        break;
    }
    invoke("create_schema", { name: uuid, schema, extra })
      .then(() => {
        showNotification("success", "create  ok");
        handleClose();
        handleReload();
      })
      .catch(e => {
        showNotification("error", "create failure", e);
      });
  };

  if (kind === DataSourceKind.Cassandra) {
    //
    return (
      <Stack>
        <RadioGroup
          row
          onChange={e => {
            setReplicationStrategy(e.target.value);
          }}
          value={replicationStrategy}
        >
          {replicationStrategies.map(e => (
            <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
          ))}
        </RadioGroup>
        <TextField
          type="number"
          value={replicationFactor}
          onChange={e => {
            setReplicationFactor(e.target.value);
          }}
          label="replication factor"
        />
        <TextField
          value={schema}
          onChange={e => {
            setSchema(e.target.value);
          }}
          inputRef={schemaDom}
          label="name"
        />
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }
  if (kind === DataSourceKind.Redis) {
    // redis have special create
    const range = [];
    for (let i = 0; i < 16; i++) {
      range.push("db" + i);
    }
    return (
      <Stack>
        <Autocomplete
          options={range.filter(e => {
            return !schemaSet.has(e);
          })}
          freeSolo={false}
          fullWidth
          value={schema}
          renderInput={params => <TextField {...params} label="database" />}
          onChange={(_, schema) => {
            setSchema(schema);
          }}
        />
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }

  return (
    <Stack>
      <Stack>
        <TextField
          value={schema}
          onChange={e => {
            setSchema(e.target.value);
          }}
          label="name"
          inputRef={schemaDom}
        />
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          create
        </Button>
      </Stack>
    </Stack>
  );
}

function SimpleFlexEditor(props: {
  language: string;
  value?: string;
  handleValue?: (x: string) => void;
  barLeft?: ReactNode;
  readOnly?: boolean;
  errPos?: Position;
  disableEditorBar?: boolean;
  handleEditor?: (editor: editor.IStandaloneCodeEditor) => void;
  dontFocus?: boolean;
}) {
  const {
    language,
    value,
    handleValue,
    barLeft,
    readOnly,
    errPos,
    handleEditor,
    disableEditorBar,
    dontFocus,
  } = props;
  const [pos, setPos] = useState<Position>(null);
  const setting = useContext(SettingContext);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const handleDidMount = useCallback((ed: editor.IStandaloneCodeEditor) => {
    setEditor(ed);
  }, []);
  useEffect(() => {
    if (!editor) {
      return;
    }
    if (!dontFocus) {
      editor.focus();
    }
    if (handleEditor) {
      handleEditor(editor);
    }
    const d1 = editor.onDidChangeCursorPosition(() => {
      setPos(editor.getPosition());
    });
    const d2 = editor.onDidChangeModelContent(() => {
      if (handleValue) {
        handleValue(editor.getValue());
      }
    });
    return () => {
      d1.dispose();
      d2.dispose();
    };
  }, [dontFocus, editor, handleEditor, handleValue]);

  useEffect(() => {
    if (editor && errPos) {
      editorSetPositionAndReveal(editor, errPos);
    }
  }, [editor, errPos]);

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    const { fontSize, fontFamily, fontWeight } = setting.appearance;
    return {
      fontSize,
      fontWeight,
      fontFamily,
      readOnly,
      tabSize: setting.editor.tabSize,
      ...setting.editor.editorOption,
    };
  }, [readOnly, setting]);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        flex: 1,
      }}
    >
      <div style={{ flex: 1 }}>
        <Editor
          width="99%"
          height="99%" // 不能设置成100%好像会影响父的高度
          onMount={handleDidMount}
          language={language}
          value={value}
          theme={setting.editorTheme()}
          options={options}
        />
      </div>
      {!disableEditorBar && <EditorBar left={barLeft} pos={pos} />}
    </div>
  );
}

function EditorBar(props: { left?: any; pos: Position }) {
  const { left, pos } = props;
  return (
    <Stack justifyContent="space-between" direction="row" alignItems="center">
      {left ? left : <span />}

      <Stack direction="row" style={{ paddingLeft: "2rem", paddingRight: "3rem" }}>
        <span>Ln {pos ? pos.lineNumber : ""}</span>
        <span>Col {pos ? pos.column : ""}</span>
      </Stack>
    </Stack>
  );
}

function MenuConnectionInfo({ uuid }) {
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const handleInfo = async () => {
    setTabs(e => {
      setTimeout(() => {
        setSelectTab(e.length + 1);
      }, 100);
      return [
        ...e,
        new TabContent(
          generateRandomString(),
          uuid,
          <ConnConfigInfo uuid={uuid} />,
          <IoMdInformationCircleOutline className="tab-icon-size" />,
        ),
      ];
    });
  };
  return (
    <MenuItem onClick={handleInfo}>
      <ListItemIcon>
        <InfoIcon />
      </ListItemIcon>
      <ListItemText> Info </ListItemText>
    </MenuItem>
  );
}

function ConnConfigInfo(props: { uuid }) {
  const { uuid } = props;
  const { showNotification } = useContext(NotificationContext);
  const [metas, setMetas] = useState<types.MetaValue[]>([]);
  const handleInfo = useCallback(
    async (showOK: boolean) => {
      try {
        const metas = await invoke<types.MetaValue[]>("server_metas", {
          name: uuid,
        });
        const config = await ConnectionHandler.getByName(uuid);
        let configMetas = [];
        switch (config.conn_type) {
          case DataSourceKind.Mysql:
          case DataSourceKind.MariaDB:
          case DataSourceKind.TiDB:
          case DataSourceKind.Postgres:
          case DataSourceKind.Cockroach:
          case DataSourceKind.Redis:
          case DataSourceKind.Mongo:
          case DataSourceKind.Cassandra:
          case DataSourceKind.Oracle:
          case DataSourceKind.ClickHouse:
          case DataSourceKind.FireBird:
          case DataSourceKind.SqlServer:
            configMetas = [
              { name: "host", value: config.host } as types.MetaValue,
              { name: "port", value: config.port + "" } as types.MetaValue,
              { name: "user", value: config.username } as types.MetaValue,
            ];
            break;
          case DataSourceKind.LibSql:
            if (config.source === "local") {
              configMetas = [{ name: "host", value: config.path } as types.MetaValue];
            } else if (config.source == "remote") {
              configMetas = [
                { name: "url", value: config.url } as types.MetaValue,
                { name: "path", value: config.path } as types.MetaValue,
              ];
            }
            break;
          case DataSourceKind.Sqlite:
            configMetas = [{ name: "path", value: config.path } as types.MetaValue];
            break;
        }
        setMetas([...configMetas, ...metas]);
        if (showOK) {
          reloadOK(showNotification);
        }
      } catch (e) {
        showNotification("error", "call server metas failed", e);
      }
    },
    [showNotification, uuid],
  );
  useEffect(() => {
    handleInfo(false);
  }, [handleInfo]);

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row">
        <Button
          color="primary"
          onClick={() => {
            handleInfo(true);
          }}
        >
          reload
        </Button>
      </Stack>

      <MetaValueView values={metas} />
    </Stack>
  );
}

function LoadStoreFile(props: {
  getQuery: () => string;
  handleLoadFile: (path: string, content: string) => void;
  kind: DataSourceKind;
}) {
  const { getQuery, handleLoadFile, kind } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const active = useContext(TabActiveContext);

  const filters = useMemo(() => {
    let filters: DialogFilter[];
    switch (kind) {
      case DataSourceKind.Mongo:
        filters = [
          {
            name: "javscript or typescrpt",
            extensions: ["ts", "js"],
          },
        ];
        break;
      case DataSourceKind.Redis:
        filters = [
          {
            name: "redis",
            extensions: ["redis"],
          },
        ];
        break;
      case DataSourceKind.Cassandra:
        filters = [
          {
            name: "cql",
            extensions: ["cql"],
          },
        ];
        break;
      default:
        filters = [
          {
            name: "SQL",
            extensions: ["sql"],
          },
        ];
    }
    return filters;
  }, [kind]);

  const handleLoad = useCallback(async () => {
    // ok to load
    try {
      const file = await open({ title: "open file", filters });
      if (file) {
        const content = new TextDecoder().decode(await readFile(file));
        handleLoadFile(file, content);
        setFile(file);
      }
    } catch (e) {
      showNotification("error", "error to open file", e);
    }
  }, [filters, handleLoadFile, showNotification]);

  const handleSave = useCallback(async () => {
    const content = getQuery();
    if (!content) {
      showNotification("warning", `can't save empty content`);
      return;
    }
    try {
      const ofile = file ? file : await save({ title: "save query", filters });
      if (ofile) {
        await writeTextFile(ofile, content, {
          create: true,
        });
        setFile(ofile);
        handleLoadFile(ofile, content);
        showNotification("success", "save file OK");
      }
    } catch (e) {
      showNotification("error", "save file failed", e);
    }
  }, [file, filters, getQuery, handleLoadFile, showNotification]);

  const handleClose = async () => {
    try {
      const fileContent = new TextDecoder().decode(await readFile(file));
      if (fileContent != getQuery()) {
        const confirm = await secondConfirm({
          title: "file not saved",
          content: "discard the content",
        });
        if (!confirm) {
          return;
        }
      }
      setFile("");
      handleLoadFile("", "");
    } catch (e) {
      showNotification("error", "close file failed", e);
    }
  };

  useEffect(() => {
    const callback = (e: KeyboardEvent) => {
      if (!active) {
        return;
      }
      if (KeyPressed.isFileSave(e)) {
        e.preventDefault();
        handleSave();
      }
      if (KeyPressed.isFileOpen(e)) {
        e.preventDefault();
        handleLoad();
      }
    };
    document.addEventListener("keydown", callback);
    return () => {
      document.removeEventListener("keydown", callback);
    };
  }, [active, file, getQuery, handleLoad, handleLoadFile, handleSave]);

  return (
    <>
      {file && (
        <StackRowCenter>
          <Tooltip title={file}>
            <Typography>{extractBaseName(file)}</Typography>
          </Tooltip>
          <Tooltip title="close file">
            <IconButton onClick={handleClose}>
              <CloseIcon />
            </IconButton>
          </Tooltip>
        </StackRowCenter>
      )}
      <Tooltip
        title={`attach file,keyboard shortcut is ${
          isMacOs() ? "command+o" : "control+o"
        }`}
      >
        <IconButton onClick={handleLoad} color="primary">
          <AttachFileIcon />
        </IconButton>
      </Tooltip>
      <Tooltip
        title={`save query to file,keyboard shortcut is ${
          isMacOs() ? "command+s" : "control+s"
        }`}
      >
        <IconButton onClick={handleSave} color="primary">
          <SaveIcon />
        </IconButton>
      </Tooltip>
    </>
  );
}

const MarkdownBody = styled("div")(({ theme }) => ({
  // ---- 裸标签 reset + 留一点空隙 ----
  "& p": { margin: "0 0 0.5em 0", lineHeight: 1.45 }, // 仅下边距 0.5em
  "& h1,& h2,& h3,& h4,& h5,& h6": { margin: "0 0 0.35em 0" }, // 标题更紧凑
  "& ul,& ol": { margin: "0 0 0.5em 0", paddingLeft: theme.spacing(2) },
  "& blockquote": {
    margin: "0 0 0.5em 0",
    paddingLeft: theme.spacing(2),
    borderLeft: `4px solid ${theme.palette.divider}`,
  },
  // ---- 其余保持原样 ----
  "& code": {
    ...theme.typography.body2,
    backgroundColor: theme.palette.action.selected,
    padding: "2px 4px",
    borderRadius: 4,
  },
}));

function AppMarkDown(props: {
  children: string;
  handlePathUrl?: (href: string) => void;
  placeHolder?: string;
}) {
  const { children, handlePathUrl, placeHolder } = props;
  function CustomLink(props: { href: string; children: ReactNode }) {
    const { href, children } = props;
    const { showNotification } = useContext(NotificationContext);
    const handleClick = event => {
      event.preventDefault();
      // 在这里处理链接点击，比如使用 Tauri API 打开链接
      console.info(`Link clicked: ${href}`);
      if (href.startsWith("http")) {
        openUrl(href)
          .then(() => {
            showNotification("success", "open url OK");
          })
          .catch(e => {
            showNotification("error", "open url failed", e);
          });
      } else {
        if (handlePathUrl) {
          handlePathUrl(href);
        } else {
          message(
            `sorry about this markdown implementation is limited,can't goto this path '${href}'.`,
            {
              kind: "error",
            },
          );
        }
      }
    };
    return (
      <a href={href} title={href} onClick={handleClick}>
        {children}
      </a>
    );
  }
  return (
    <MarkdownBody>
      <ReactMarkdown
        components={{
          code({ className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || "");
            return match ? (
              <SyntaxHighlighter
                style={syntaxHightLightTheme()} // Apply the chosen theme
                language={match[1]}
                PreTag="div"
                {...props}
              >
                {String(children).replace(/\n$/, "")}
              </SyntaxHighlighter>
            ) : (
              <code className={className} {...props}>
                {children}
              </code>
            );
          },
          a: CustomLink,
        }}
      >
        {children ? children : placeHolder}
      </ReactMarkdown>
    </MarkdownBody>
  );
}

function syntaxHightLightTheme() {
  return isDarkTheme() ? oneDark : oneLight;
}

function UrlDocument(props: { url: string }) {
  const { url } = props;
  return (
    <Tooltip title={url}>
      <InfoIcon
        href={url}
        style={{ cursor: "pointer" }}
        onClick={e => {
          e.preventDefault();
          openUrl(url);
        }}
      />
    </Tooltip>
  );
}

function SaveFavoriteSqlView(props: {
  favoriteName?: string;
  editor: editor.IStandaloneCodeEditor;
  uuid: string;
  language?: string;
  kind: DataSourceKind;
}) {
  const { favoriteName: oldName, editor, uuid, language, kind } = props;
  const [favoriteName, setFavoriteName] = useState(oldName);
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  return (
    <>
      <Tooltip
        title={`save favorite sql ${favoriteName && `as named '${favoriteName}'`}`}
      >
        <IconButton
          onClick={async () => {
            const sql = editor.getValue();
            if (!sql) {
              showNotification("warning", "can't save empty statement.");
              return;
            }
            if (favoriteName) {
              new FavoriteSql(uuid, favoriteName, kind, sql, language).save(
                showNotification,
              );
            } else {
              setShowFavoriteName(true);
            }
          }}
          color="primary"
        >
          <FaBookmark style={{ fontSize: "1.2rem" }} />
        </IconButton>
      </Tooltip>

      {showSetFavoriteName && (
        <Modal
          open={showSetFavoriteName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFavoriteName(false);
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader
              title="edit favorite sql name"
              handleClose={() => {
                setShowFavoriteName(false);
              }}
            />
            <Stack>
              <TextField
                fullWidth
                label="name"
                value={favoriteName}
                onChange={e => {
                  setFavoriteName(e.target.value);
                }}
                required
              />
              <Stack direction="row">
                <IconButton
                  color="primary"
                  onClick={() => {
                    if (!favoriteName) {
                      showNotification("warning", "name can't be empty");
                      return;
                    }
                    setShowFavoriteName(false);
                  }}
                >
                  <FaBookmark> </FaBookmark>
                </IconButton>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
    </>
  );
}
function CancelButton({ handleCancel }) {
  return (
    <IconButton
      color="warning"
      onClick={() => {
        handleCancel();
      }}
    >
      <CancelIcon />
    </IconButton>
  );
}

function getSliderSizeStoreValue(storeKey: string, defaultValue: number) {
  const keyXxx = "dbpal-max-row-" + storeKey;
  return parseInt(localStorage.getItem(keyXxx)) || defaultValue;
}

function SliderSize(props: {
  storeKey: string;
  notice: ReactNode;
  min: number;
  max: number;
  step: number;
  handleValue: (x: number) => void;
  name?: string;
}) {
  const { storeKey, handleValue, notice, min, max, step, name } = props;
  const width = "450px";
  const keyXxx = "dbpal-max-row-" + storeKey;
  const [maxRow, setMaxRow] = useState(getSliderSizeStoreValue(storeKey, min));
  useEffect(() => {
    localStorage.setItem(keyXxx, maxRow + "");
    handleValue(maxRow);
  }, [maxRow, keyXxx, handleValue]);
  return (
    <Tooltip
      slotProps={{
        tooltip: {
          style: { maxWidth: width, padding: 0, margin: 0 },
        },
      }}
      title=<Paper
        style={{
          width,
          padding: "8px",
        }}
      >
        <Stack>
          <Slider
            value={maxRow}
            min={min}
            max={max}
            onChange={(_, v) => {
              setMaxRow(v);
            }}
            color="success"
            step={step}
          />
          {notice}
        </Stack>
      </Paper>
    >
      <Chip label={`${name ? name + ":" : ""} ${maxRow}`} color="info" />
    </Tooltip>
  );
}

function TableAddButton({ handleAdd }) {
  return (
    <IconButton onClick={handleAdd} color="primary">
      <AddIcon />
    </IconButton>
  );
}

// select column include asc or desc
// useful for create index
function SelectColumnAsc(props: {
  fields: string[];
  includes: types.CreateIndexColumn[];
  setIncludes;
  kind: DataSourceKind;
}) {
  const { fields, includes, setIncludes, kind } = props;
  const [value, setValue] = useState("");

  const supportOrder = ![DataSourceKind.Cassandra, DataSourceKind.FireBird].includes(
    kind,
  );
  return (
    <Stack>
      {includes.length == 0 && (
        <Typography color="warning"> need select at least one column </Typography>
      )}
      <Stack>
        {includes.map((f, index) => {
          return (
            <Stack
              direction="row"
              style={{
                justifyContent: "space-between",
                alignItems: "center",
                padding: "2px 4px",
                border: "1px solid grey",
                borderRadius: "10px",
              }}
              key={f.col}
            >
              <Typography color="info" style={{ width: "55%" }}>
                {f.col}
              </Typography>
              {supportOrder && (
                <FormControlLabel
                  label="asc"
                  control={
                    <Checkbox
                      checked={f.asc}
                      onChange={e => {
                        includes[index].asc = e.target.checked;
                        setIncludes([...includes]);
                      }}
                    />
                  }
                />
              )}
              <IconButton
                onClick={() => {
                  let i = includes.filter(x => x.col != f.col);
                  setIncludes(i);
                }}
              >
                <CancelIcon> </CancelIcon>
              </IconButton>
            </Stack>
          );
        })}
      </Stack>
      <Autocomplete
        style={{ flex: 1 }}
        options={fields.filter(e => {
          return !includes.find(f => f.col == e);
        })}
        freeSolo={false}
        value={value}
        onChange={(_, value) => {
          setValue(value);
          if (value) {
            const index = includes.findIndex(e => e.col == value);
            if (index == -1) {
              setIncludes([
                ...includes,
                { col: value, asc: null } as types.CreateIndexColumn,
              ]);
            }
          }
        }}
        renderInput={params => <TextField {...params} label="select columns" />}
      />
    </Stack>
  );
}

function CreateIndexReq(props: {
  fields: string[];
  handleReq: (x: types.CreateIndexReq) => void;
  kind: DataSourceKind;
  style?: CSSProperties;
}) {
  const { handleReq, kind, fields, style } = props;
  const indexTypes = ["index", "unique"];
  const [req, setReq] = useState<types.CreateIndexReq>({
    name: "",
    columns: [],
    kind: "index",
  });
  useEffect(() => {
    handleReq(req);
  }, [handleReq, req]);

  let indexTypeJsx: ReactNode;
  switch (kind) {
    case DataSourceKind.Cassandra:
      break;
    default:
      indexTypeJsx = (
        <>
          <FormLabel component="legend">unique</FormLabel>
          <RadioGroup
            row
            onChange={e => {
              setReq({ ...req, kind: e.target.value as types.IndexKind });
            }}
            value={req.kind}
          >
            {indexTypes.map(e => (
              <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
            ))}
          </RadioGroup>
        </>
      );
      break;
  }
  return (
    <Stack style={style}>
      <TextField
        value={req.name}
        onChange={e => {
          setReq({ ...req, name: e.target.value });
        }}
        label="index name"
        required
        slotProps={{
          input: {
            endAdornment: req.name && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setReq({ ...req, name: "" });
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
      <SelectColumnAsc
        includes={req.columns}
        setIncludes={rs => {
          setReq({ ...req, columns: rs });
        }}
        kind={kind}
        fields={fields}
      />
      {indexTypeJsx}
    </Stack>
  );
}
function TypeSelect(props: { handleChange; uuid; type; style?: CSSProperties }) {
  const { handleChange, uuid, type, style } = props;
  const [types, setTypes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string[]>("list_data_types", { name: uuid })
      .then(ts => {
        setTypes(ts);
      })
      .catch(e => {
        showNotification("error", "should not return a error", e);
      });
  }, [showNotification, uuid]);
  return (
    <Autocomplete
      freeSolo
      options={types}
      style={{ width: "22rem", ...style }}
      value={type}
      renderInput={params => (
        <TextField {...params} label={`type def may require size varchar(100)`} />
      )}
      onChange={(_, t) => {
        handleChange(t);
      }}
      onInputChange={(_, t) => {
        handleChange(t);
      }}
    />
  );
}

function ColumnNullness(props: {
  nullable: types.SqlNullAble;
  handleNullable: (_: types.SqlNullAble) => void;
}) {
  const { nullable, handleNullable: handleNullness } = props;
  const opts: types.SqlNullAble[] = ["null", "notnull", "none"];
  return (
    <Autocomplete
      value={nullable}
      options={opts}
      style={{ width: "17rem" }}
      renderInput={params => <TextField {...params} label="specify null" />}
      onChange={(_, value) => {
        handleNullness(value);
      }}
      freeSolo={false}
    />
  );
}

function SqlDefaultValue(props: { uuid: string; defaultValue; setDefaultValue }) {
  const { uuid, defaultValue, setDefaultValue } = props;
  const [currentValue, setCurrentValue] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string[]>("list_sql_current_values", { name: uuid })
      .then(ts => {
        setCurrentValue(ts);
      })
      .catch(e => {
        showNotification("success", "list sql current values failed", e);
      });
  }, [showNotification, uuid]);

  return (
    <Stack direction="row">
      <Autocomplete
        freeSolo
        options={currentValue}
        style={{ width: "14rem" }}
        value={defaultValue}
        renderInput={params => <TextField {...params} label="default value" />}
        onChange={(_, value) => {
          setDefaultValue(value);
        }}
        onInputChange={(_, value) => {
          setDefaultValue(value);
        }}
      />
    </Stack>
  );
}

function AppDataGridPagination(props: { showPageSelect?: boolean }) {
  const { showPageSelect } = props;
  const apiRef = useGridApiContext();
  const paginationModel = useGridSelector(apiRef, gridPaginationModelSelector);
  const pageCount = useGridSelector(apiRef, gridPageCountSelector);
  return (
    <>
      <Pagination
        count={pageCount}
        page={paginationModel.page + 1}
        color="primary"
        onChange={(_, value) => apiRef.current.setPage(value - 1)}
      />
      {showPageSelect && (
        <SelectPageSize
          pageSize={apiRef.current.state.pagination.paginationModel.pageSize}
          handleChange={pageSize => {
            apiRef.current.setPageSize(pageSize);
          }}
        />
      )}
    </>
  );
}

function LastFetchTime(props: { date: Date }) {
  const { date } = props;
  return <Typography color="info"> {date.toLocaleString()} </Typography>;
}

function AppDataGridToolBar(props: {
  title: string;
  left?: ReactNode;
  right?: ReactNode;
  noWrapLeft?: boolean; // don't wrap left in `Stack` node
}) {
  const { title, left, right, noWrapLeft } = props;
  return (
    <Stack
      direction="row"
      style={{
        alignItems: "center",
        justifyContent: "space-between",
        border: "1px solid grey",
        borderRadius: "4px",
        padding: "2px",
      }}
    >
      {noWrapLeft ? left : <StackRowCenter>{left}</StackRowCenter>}
      <Typography variant="h5">{title}</Typography>
      <Stack direction="row" style={{ alignItems: "center" }}>
        {right}
        <CustomToolbar />
      </Stack>
    </Stack>
  );
}

function QueryStatus(props: { setRef: RefObject<SetStatusFn> }) {
  const { setRef } = props;
  const [queryStatus, setQueryStatus] = useState("");
  useEffect(() => {
    setRef.current = s => {
      setQueryStatus(s);
    };
  }, [setRef]);

  useEffect(() => {
    console.log(new Date().toLocaleString(), "query status", queryStatus);
  }, [queryStatus]);
  return (
    queryStatus && (
      <Stack
        direction="row"
        style={{ position: "absolute", bottom: 0, left: 0, opacity: 0.5, zIndex: 14 }}
      >
        <CircularProgress />
        <Typography> {queryStatus} </Typography>
      </Stack>
    )
  );
}

function StackRowCenter(props: { children: ReactNode; style?: CSSProperties }) {
  const { children, style } = props;
  return (
    <Stack style={{ alignItems: "center", ...style }} direction="row">
      {children}
    </Stack>
  );
}

function makeExtractTableSelectWhere(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_selection", {
      name: uuid,
      sql,
    });
  };
}

function makeExtractTableSelectSort(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_sort", {
      name: uuid,
      sql,
    });
  };
}
function SetFilterSort(props: {
  open: boolean;
  handleClose: () => void;
  value: string;
  setValue;
  label: "filter" | "sort";
  tableName: string;
  kind: DataSourceKind;
  uuid: string;
}) {
  let { open, handleClose, value, setValue, label, tableName, kind, uuid } = props;
  const isFilter = label == "filter";
  const extractFn = isFilter
    ? makeExtractTableSelectWhere(uuid)
    : makeExtractTableSelectSort(uuid);
  tableName = quoteDatabaseIdentifier(kind)(tableName);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [modalBox, setModalBox] = useState(null);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "where-sort");
      return clean;
    }
  }, [modalBox]);

  const [selectFilter, setSelectFilter] = useState("");
  const [filters, setFileters] = useState<TableFilter[]>([]);
  const loadFilter = useCallback(async () => {
    const x = await TableFilterHandler.get(uuid, tableName);
    if (x) {
      setFileters(x);
    }
  }, [tableName, uuid]);
  useEffect(() => {
    if (!isFilter) {
      return;
    }
    loadFilter();
  }, [isFilter, loadFilter]);

  const formatarg = {
    tabSize: setting.editor.tabSize,
    insertSpaces: true,
  };

  const handleSet = async () => {
    try {
      let value = await extractFn(editor.getValue());
      if (value == null) {
        value = "";
      }
      setValue(value);
      handleClose();
    } catch (e) {
      showNotification("error", "extract sql error.", e);
      return;
    }
  };
  return (
    <Modal
      open={open}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}
    >
      <Box
        sx={{ resize: "both", ...modalStyle }}
        width="75%"
        height="30vh"
        ref={setModalBox}
      >
        <ModalHeader title={`set ${label}`} handleClose={handleClose} />
        <SimpleFlexEditor
          value={
            makeFormatFn(kind)(
              isFilter
                ? `SELECT * FROM ${tableName} WHERE ${value}`
                : `SELECT * FROM ${tableName} ORDER BY ${value}`,
              formatarg,
            ) + " "
          }
          handleEditor={e => {
            setEditor(e);
            e.focus();
            editorPositionToEnd(e);
            SetAutoCompletionName.set(e.getModel().id, new NameAndQueryId(uuid, kind));
          }}
          language={getLanguageFromDataSourceKind(kind)}
        />

        <StackRowCenter>
          <Button onClick={handleSet}>set {label}</Button>
          <Button
            onClick={() => {
              editor.setValue(`select * from ${tableName}`);
            }}
            variant="outlined"
          >
            clear
          </Button>
          {isFilter && (
            <Tooltip title="save current value to filter bookmark">
              <IconButton
                onClick={async () => {
                  const value = editor.getValue();
                  if (!value) {
                    showNotification("warning", "this editor is empty");
                    return;
                  }
                  try {
                    const filter = await extractFn(value);
                    await TableFilterHandler.add(uuid, tableName, filter);
                    loadFilter();
                  } catch (e) {
                    showNotification("error", "save filter failed", e);
                  }
                }}
              >
                <SaveIcon />
              </IconButton>
            </Tooltip>
          )}
          {isFilter && (
            <Autocomplete
              value={selectFilter}
              style={{ flex: 1 }}
              options={filters.map(e => e.filter)}
              freeSolo={false}
              onChange={async (_, value) => {
                editor.setValue(
                  makeFormatFn(kind)(
                    `SELECT * FROM ${tableName} WHERE ${value}`,
                    formatarg,
                  ),
                );
                setSelectFilter(value);
              }}
              renderInput={params => <TextField {...params} label="filters" />}
            />
          )}
        </StackRowCenter>
      </Box>
    </Modal>
  );
}
export {
  CreateDatabase,
  CreateSchema,
  SimpleFlexEditor,
  EditorBar,
  MenuConnectionInfo,
  ConnConfigInfo,
  LoadStoreFile,
  AppMarkDown,
  UrlDocument,
  syntaxHightLightTheme,
  SaveFavoriteSqlView,
  CancelButton,
  SliderSize,
  TableAddButton,
  SelectColumnAsc,
  CreateIndexReq,
  TypeSelect,
  ColumnNullness,
  SqlDefaultValue,
  AppDataGridPagination,
  AppDataGridToolBar,
  LastFetchTime,
  QueryStatus,
  StackRowCenter,
  getSliderSizeStoreValue,
  SetFilterSort,
};
