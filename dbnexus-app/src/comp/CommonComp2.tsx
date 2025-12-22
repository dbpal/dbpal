import {
  ReactNode,
  RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { NotificationContext } from "./Notification";
import Stack from "@mui/material/Stack";
import InfoIcon from "@mui/icons-material/Info";
import CloseIcon from "@mui/icons-material/Close";
import SaveIcon from "@mui/icons-material/Save";
import { FaBookmark } from "react-icons/fa";
import CancelIcon from "@mui/icons-material/Cancel";
import AddIcon from "@mui/icons-material/Add";
import {
  Autocomplete,
  Box,
  Button,
  Chip,
  CircularProgress,
  FormControlLabel,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Modal,
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
  editorSetPositionAndReveal,
  FavoriteSql,
  generateRandomString,
  isDarkTheme,
  isMacOs,
  KeyPressed,
  secondConfirm,
  SettingContext,
  SomeTempStore,
  TabActiveContext,
} from "./helper";
import { Editor } from "@monaco-editor/react";
import { editor, Position } from "monaco-editor";
import { invoke } from "@tauri-apps/api/core";
import {
  ConnectionHandler,
  EscDownEvent,
  extractBaseName,
  modalHandleClose,
  modalStyle,
  reloadOK,
} from "./Common";
import types from "./ApiStruct";
import { MetaValueView, ModalHeader } from "./CommonComp";
import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { save, open, DialogFilter, message } from "@tauri-apps/plugin-dialog";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import ReactMarkdown from "react-markdown";
import { oneDark, oneLight } from "react-syntax-highlighter/dist/esm/styles/prism";
import { styled } from "@mui/material/styles";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { openUrl } from "@tauri-apps/plugin-opener";
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

const infoWindows = new Map<string, WebviewWindow>();

function MenuConnectionInfo({ uuid }) {
  const { showNotification } = useContext(NotificationContext);
  const handleInfo = async () => {
    const old = infoWindows.get(uuid);
    if (old) {
      old.setFocus();
      return;
    }
    SomeTempStore.set(uuid);
    const webview = new WebviewWindow(generateRandomString(), {
      title: `some infomation about '${uuid}'`,
      url: "info.html",
      width: 800,
      height: 600,
    });
    webview.once("tauri://created", () =>
      console.info("create info view successfully"),
    );
    webview.once("tauri://error", e => {
      showNotification("error", "create webview failed", e.event.toString());
    });
    infoWindows.set(uuid, webview);
    webview.onCloseRequested(() => {
      infoWindows.delete(uuid);
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
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Tooltip title={file}>
            <Typography>{extractBaseName(file)}</Typography>
          </Tooltip>
          <Tooltip title="close file">
            <IconButton onClick={handleClose}>
              <CloseIcon />
            </IconButton>
          </Tooltip>
        </Stack>
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
function showProgressingStatus(showNotification, status: string) {
  showNotification(
    "info",
    "server is doing this.",
    <Stack direction="row">
      <CircularProgress />
      <Typography color="info"> {status} </Typography>
    </Stack>,
  );
}

function SliderSize(props: {
  storeKey: string;
  valueRef: RefObject<number>;
  notice: ReactNode;
  min: number;
  max: number;
  step: number;
}) {
  const { storeKey, valueRef, notice, min, max, step } = props;
  const width = "450px";
  const storekey = "dbpal-max-row-" + storeKey;
  const [maxRow, setMaxRow] = useState(parseInt(localStorage.getItem(storekey)) || min);
  useEffect(() => {
    localStorage.setItem(storekey, maxRow + "");
    valueRef.current = maxRow;
  }, [maxRow, valueRef, storekey]);
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
      <Chip label={maxRow} color="info" />
    </Tooltip>
  );
}

function TableAddButton({ handleAdd }) {
  return (
    <Button onClick={handleAdd} style={{ fontSize: "1rem" }}>
      <AddIcon> </AddIcon>
    </Button>
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
  showProgressingStatus,
  SliderSize,
  TableAddButton,
};
