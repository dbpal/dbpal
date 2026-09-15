import {
  CSSProperties,
  Dispatch,
  memo,
  ReactNode,
  SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import wcmatch from "wildcard-match";
import {
  ClearIcon,
  InfoIcon,
  CloseIcon,
  SaveIcon,
  CancelIcon,
  ChatBubbleOutlineOutlinedIcon,
  ContentCopyIcon,
  CircleIcon,
  DeleteForeverIcon,
  EventNoteIcon,
  DownloadIcon,
  AttachFileIcon,
  ChatBubbleIcon,
  EditIcon,
  FaBookmark,
  FaCopy,
  IoMdInformationCircleOutline,
  LuRefreshCcw,
  IoTimerOutline,
  MdTimer,
} from "./icons";
import Stack from "@mui/material/Stack";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Divider,
  FormControlLabel,
  FormLabel,
  IconButton,
  IconButtonProps,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  MenuList,
  Modal,
  Pagination,
  Paper,
  Radio,
  RadioGroup,
  Slider,
  Switch,
  TextField,
  TextFieldProps,
  Tooltip,
  Typography,
} from "@mui/material";

import {
  AppCodeExtraUI,
  AppCodeHanleEdit,
  arrayToMarkdownTable,
  ChatShowContext,
  createCSV,
  DataSourceKind,
  editorPositionToEnd,
  generateDefaultFilename,
  handleCopyToClipboard,
  isMacOs,
  KeyPressed,
  parseCellType,
  prettyMs,
  quoteDatabaseIdentifier,
  secondConfirm,
  TabActiveContext,
  TabContent,
  textEllipsisStyle,
  DbnameContext,
  isTypeLargeText,
} from "./helper";
import { editor } from "monaco-editor";
import {
  EscDownEvent,
  extractBaseName,
  formatSqlFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  reloadOK,
} from "./Common";

import {
  MetaValueView,
  ModalHeader,
  OpenFileWithDefaultApp,
  SelectPageSize,
} from "./CommonComp";
import {
  FileHandle,
  readFile,
  writeTextFile,
  open as openFile,
} from "@tauri-apps/plugin-fs";
import { save, open, DialogFilter, message } from "@tauri-apps/plugin-dialog";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { styled, useTheme } from "@mui/material/styles";
import { openUrl } from "@tauri-apps/plugin-opener";

import { CustomToolbar } from "./CustomToolBar";
import {
  passwordKey,
  useBoolSwitch,
  useConnectionStore,
  useElementSizeStore,
  useFavoriteStore,
  useLLM,
  useNotification,
  useOneShotDialogUI,
  UseSizeConfig,
  useSizeStore,
  useSqlDataFilter,
  useTableFilter,
  useTabSize,
  useTabsManage,
} from "./store";
import { MonacoEditor } from "./Editor";

import { Highlight, themes } from "prism-react-renderer";
import { visit, SKIP } from "unist-util-visit";
import {
  commands,
  CouchBaseBucketType,
  CreateDatabaseExtra,
  CreateIndexColumn,
  CreateIndexReq,
  Field,
  IndexKind,
  MetaValue,
  SqlNullAble,
  XlsxCellData,
} from "./bindings";
import {
  gridPageCountSelector,
  gridPaginationModelSelector,
  gridRowsLoadingSelector,
  useGridApiContext,
  useGridSelector,
} from "./datagridExport";
import { QueryLog } from "./QueryLog";

interface ClearableTextFieldProps extends Omit<TextFieldProps, "variant"> {
  onClear: () => void;
}

export function ClearableTextField({
  value,
  onClear,
  ...props
}: ClearableTextFieldProps) {
  return (
    <TextField
      value={value}
      {...props}
      // We "merge" slotProps so you can still pass other custom input props if needed
      slotProps={{
        ...props.slotProps,
        input: {
          ...props.slotProps?.input,
          endAdornment: !props.disabled && value && (
            <InputAdornment position="end">
              <IconButton onClick={onClear} edge="end">
                <ClearIcon fontSize="small" />
              </IconButton>
            </InputAdornment>
          ),
        },
      }}
    />
  );
}

function AppCode_(props: {
  codeBlock: string;
  language: string;
  extra?: AppCodeExtraUI;
  handleEdit?: AppCodeHanleEdit;
  highlightStr?: string;
  style?: CSSProperties;
}) {
  const theme = useTheme();
  const { codeBlock, language, extra, handleEdit, highlightStr, style } = props;
  const { showNotification } = useNotification();

  return (
    <Paper
      style={{
        padding: "2px 4px",
        borderRadius: "0.4rem",
        margin: 0,
        overflow: "auto",
        minWidth: "100%",
        width: "fit-content",
        ...style,
      }}
      elevation={10}>
      <Stack
        direction="row"
        style={{
          alignItems: "center",
          position: "sticky",
          top: 0,
          backgroundColor: theme.palette.background.paper,
        }}>
        <IconButton
          color="secondary"
          onClick={() => {
            handleCopyToClipboard(codeBlock, showNotification);
          }}>
          <ContentCopyIcon />
        </IconButton>
        {extra && extra({ code: codeBlock, language })}
        {handleEdit && (
          <IconButton
            onClick={() => {
              handleEdit({ code: codeBlock });
            }}
            color="secondary">
            <EditIcon />
          </IconButton>
        )}
      </Stack>

      <Highlight
        theme={theme.palette.mode == "dark" ? themes.oneDark : themes.oneLight}
        code={codeBlock}
        language={language}>
        {({ style, tokens, getLineProps, getTokenProps }) => (
          <pre style={style}>
            {tokens.map((line, i) => (
              <div key={i} {...getLineProps({ line })}>
                <span
                  style={{
                    width: "4rem",
                    textAlign: "left",
                    display: "inline-block",
                  }}>
                  {i + 1}
                </span>
                {line.map((token, key) =>
                  (function () {
                    if (
                      highlightStr &&
                      token.content.toLowerCase().includes(highlightStr)
                    ) {
                      return <mark>{token.content}</mark>;
                    }
                    return <span key={key} {...getTokenProps({ token })} />;
                  })(),
                )}
              </div>
            ))}
          </pre>
        )}
      </Highlight>
    </Paper>
  );
}

const AppCode = memo(AppCode_);

function CreateDatabase(props: {
  dbName: string;
  handleClose: () => void;
  kind: DataSourceKind;
}) {
  const { dbName, handleClose, kind } = props;
  const { showNotification } = useNotification();
  const [database, setDatabase] = useState("");
  const databaseDom = useRef(null);

  const [couchbasExtra, setCouchbaseExtra] = useState<{
    ram_quota_mb: string;
    flush_enabled: boolean;
    num_replicas: string;
    max_expiry: string;
    bucket_type: CouchBaseBucketType;
  }>({
    ram_quota_mb: "1024",
    flush_enabled: true,
    num_replicas: "2",
    max_expiry: null,
    bucket_type: "couchbase",
  });
  const handleCreate = () => {
    if (!database) {
      databaseDom.current.focus();
      showNotification({ severity: "warning", title: "database name is empty" });
      return;
    }
    let extra: CreateDatabaseExtra = null;
    switch (kind) {
      case DataSourceKind.CouchBase:
        extra = {
          extra: "couchbase",
          max_expiry: couchbasExtra.max_expiry
            ? parseInt(couchbasExtra.max_expiry)
            : null,
          ram_quota_mb: parseInt(couchbasExtra.ram_quota_mb),
          flush_enabled: couchbasExtra.flush_enabled,
          num_replicas: parseInt(couchbasExtra.num_replicas),
          bucket_type: couchbasExtra.bucket_type,
        };
    }
    commands
      .createDatabase(dbName, database, extra)
      .then(() => {
        showNotification({ severity: "success", title: "Create OK" });
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Create Failure", message: e });
      });
  };

  return (
    <Stack>
      {kind == DataSourceKind.CouchBase && (
        <>
          <Autocomplete
            options={["couchbase", "ephemeral"] as CouchBaseBucketType[]}
            style={{ flex: 1 }}
            value={couchbasExtra.bucket_type}
            renderInput={(params) => <TextField {...params} label={`bucket type`} />}
            freeSolo={false}
            onChange={(_, value) => {
              setCouchbaseExtra({ ...couchbasExtra, bucket_type: value });
            }}
          />
          <StackRowCenter>
            <ClearableTextField
              label="ram quota mb"
              value={couchbasExtra.ram_quota_mb}
              onChange={(e) =>
                setCouchbaseExtra({ ...couchbasExtra, ram_quota_mb: e.target.value })
              }
              style={{ flex: 1 }}
              error={isNaN(parseInt(couchbasExtra.ram_quota_mb))}
              onClear={() => setCouchbaseExtra({ ...couchbasExtra, ram_quota_mb: "" })}
            />
            <ClearableTextField
              label="max expiry(in seconds)"
              value={couchbasExtra.max_expiry}
              onChange={(e) =>
                setCouchbaseExtra({ ...couchbasExtra, max_expiry: e.target.value })
              }
              style={{ flex: 1 }}
              error={
                couchbasExtra.max_expiry
                  ? isNaN(parseInt(couchbasExtra.max_expiry))
                  : false
              }
              onClear={() => setCouchbaseExtra({ ...couchbasExtra, max_expiry: "" })}
            />
          </StackRowCenter>

          <Stack direction="row" style={{ alignItems: "center" }}>
            <ClearableTextField
              label="number of replicas"
              value={couchbasExtra.num_replicas}
              onChange={(e) =>
                setCouchbaseExtra({ ...couchbasExtra, num_replicas: e.target.value })
              }
              error={isNaN(parseInt(couchbasExtra.num_replicas))}
              onClear={() => setCouchbaseExtra({ ...couchbasExtra, num_replicas: "" })}
            />
            <FormControlLabel
              control={
                <Checkbox
                  checked={couchbasExtra.flush_enabled}
                  onChange={(e) =>
                    setCouchbaseExtra({
                      ...couchbasExtra,
                      flush_enabled: e.target.checked,
                    })
                  }
                />
              }
              label="flush enabled"
            />
          </Stack>
        </>
      )}

      <Stack>
        <TextField
          inputRef={databaseDom}
          value={database}
          onChange={(e) => {
            setDatabase(e.target.value);
          }}
          label="database name"
        />
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}>
          create
        </Button>
      </Stack>
    </Stack>
  );
}

/**
 * Modal used to create a new schema / keyspace / database object.
 */
function CreateSchema(props: {
  dbName: string;
  handleClose: () => void;
  handleReload: () => void;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { dbName, handleClose, kind, schemas, handleReload } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useNotification();
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
      showNotification({ severity: "error", title: "Schema is empty" });
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification({ severity: "warning", title: `'${schema}' already exists.` });
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
    commands
      .createSchema(dbName, schema, extra)
      .then(() => {
        showNotification({ severity: "success", title: "create  ok" });
        handleClose();
        handleReload();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "create failure", message: e });
      });
  };

  if (kind === DataSourceKind.Cassandra) {
    //
    return (
      <Stack>
        <RadioGroup
          row
          onChange={(e) => {
            setReplicationStrategy(e.target.value);
          }}
          value={replicationStrategy}>
          {replicationStrategies.map((e) => (
            <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
          ))}
        </RadioGroup>
        <TextField
          type="number"
          value={replicationFactor}
          onChange={(e) => {
            setReplicationFactor(e.target.value);
          }}
          label="replication factor"
        />
        <TextField
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          inputRef={schemaDom}
          label="name"
        />
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}>
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
          options={range.filter((e) => {
            return !schemaSet.has(e);
          })}
          freeSolo={false}
          fullWidth
          value={schema}
          renderInput={(params) => <TextField {...params} label="database" />}
          onChange={(_, schema) => {
            setSchema(schema);
          }}
        />
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}>
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
          onChange={(e) => {
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
          }}>
          create
        </Button>
      </Stack>
    </Stack>
  );
}

/**
 * Simple editor wrapper with optional toolbar and value callbacks.
 */
function SimpleFlexEditor(props: {
  language: string;
  value?: string;
  barLeft?: ReactNode;
  readOnly?: boolean;
  handleEditor?: (editor: editor.IStandaloneCodeEditor) => void;
  handleValue?: (value: string) => void;
}) {
  const { language, value, barLeft, readOnly, handleEditor, handleValue } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const handleDidMount = useCallback((ed: editor.IStandaloneCodeEditor) => {
    setEditor(ed);
  }, []);
  useEffect(() => {
    if (!editor) {
      return;
    }
    if (handleEditor) {
      handleEditor(editor);
    }
    const d = editor.onDidChangeModelContent(() => {});
    return () => {
      d.dispose();
    };
  }, [editor, handleEditor]);

  useEffect(() => {
    if (!editor) {
      return;
    }
    if (handleValue) {
      let d = editor.onDidChangeModelContent(() => {
        handleValue(editor.getValue());
      });
      return () => {
        d.dispose();
      };
    }
  }, [editor, handleValue]);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        flex: 1,
      }}>
      <div style={{ flex: 1 }}>
        <MonacoEditor
          onMount={handleDidMount}
          language={language}
          value={value}
          readOnly={readOnly}
          barLeft={barLeft}
        />
      </div>
    </div>
  );
}

/**
 * Menu item for connection info tab.
 */
function MenuConnectionInfo(props: { dbName: string }) {
  const { dbName } = props;
  const appendTab = useTabsManage((state) => state.appendTab);
  const handleInfo = async () => {
    appendTab(
      new TabContent(
        dbName,
        <ConnConfigInfo />,
        <IoMdInformationCircleOutline />,
        dbName,
      ),
    );
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

/**
 * Displays backend connection metadata values.
 */
function ConnConfigInfo() {
  const { dbName } = useContext(DbnameContext);
  const { showNotification } = useNotification();
  const [metas, setMetas] = useState<MetaValue[]>([]);
  const { getConnection } = useConnectionStore();

  const handleInfo = useCallback(
    async (showOK: boolean) => {
      try {
        const config = await getConnection(dbName);
        let configMetas = [];
        let metas: MetaValue[] = await commands.serverMetas(dbName).catch((e) => {
          console.error("call server metas failed", e);
          return [];
        });
        if (passwordKey in config) {
          delete config[passwordKey];
        }
        for (const key in config) {
          configMetas.push({
            name: key,
            value: JSON.stringify(config[key]),
          });
        }
        setMetas([...configMetas, ...metas]);
        if (showOK) {
          reloadOK(showNotification);
        }
      } catch (e) {
        showNotification({
          severity: "error",
          title: "get config info failed",
          message: e,
        });
      }
    },
    [getConnection, showNotification, dbName],
  );
  useEffect(() => {
    handleInfo(false);
  }, [handleInfo]);

  return (
    <Stack style={{ height: "100%" }}>
      <MetaValueView
        values={metas}
        reload={() => {
          handleInfo(true);
        }}
      />
    </Stack>
  );
}

function LoadStoreFile(props: {
  getQuery: () => string;
  handleLoadFile: (content: string) => void;
  kind: DataSourceKind;
}) {
  const { getQuery, handleLoadFile, kind } = props;
  const [file, setFile] = useState("");
  const { showNotification } = useNotification();
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
        handleLoadFile(content);
        setFile(file);
      }
    } catch (e) {
      showNotification({ severity: "error", title: "error to open file", message: e });
    }
  }, [filters, handleLoadFile, showNotification]);

  const handleSave = useCallback(async () => {
    const content = getQuery();
    if (!content) {
      showNotification({ severity: "warning", title: `can't save empty content` });
      return;
    }
    try {
      const ofile = file ? file : await save({ title: "save query", filters });
      if (ofile) {
        await writeTextFile(ofile, content, {
          create: true,
        });
        setFile(ofile);
        handleLoadFile(content);
        showNotification({ severity: "success", title: "save file OK" });
      }
    } catch (e) {
      showNotification({ severity: "error", title: "save file failed", message: e });
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
      handleLoadFile("");
    } catch (e) {
      showNotification({ severity: "error", title: "close file failed", message: e });
    }
  };

  useEffect(() => {
    const handleFile = (e: KeyboardEvent) => {
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
    document.addEventListener("keydown", handleFile);
    return () => {
      document.removeEventListener("keydown", handleFile);
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
        }`}>
        <IconButton onClick={handleLoad} color="primary">
          <AttachFileIcon />
        </IconButton>
      </Tooltip>
      <Tooltip
        title={`save query to file,keyboard shortcut is ${
          isMacOs() ? "command+s" : "control+s"
        }`}>
        <IconButton onClick={handleSave} color="primary">
          <SaveIcon />
        </IconButton>
      </Tooltip>
    </>
  );
}

const MarkdownBody = styled("div")(({ theme }) => ({
  // ---- 裸标签 reset + 留一点空隙 ----
  "& p": { margin: "0 0 0.5em 0", lineHeight: 1.45 },
  "& h1,& h2,& h3,& h4,& h5,& h6": { margin: "0 0 0.35em 0" },
  "& ul,& ol": { margin: "0 0 0.5em 0", paddingLeft: theme.spacing(2) },
  "& blockquote": {
    margin: "0 0 0.5em 0",
    paddingLeft: theme.spacing(2),
    borderLeft: `4px solid ${theme.palette.divider}`,
  },

  // ---- 新增：表格样式 (Table) ----
  "& table": {
    width: "100%",
    borderCollapse: "collapse",
    marginBottom: "1em",
    overflow: "hidden",
    display: "block", // 允许横向滚动
    overflowX: "auto", // 适配小屏幕
  },
  "& th, & td": {
    border: `1px solid ${theme.palette.divider}`,
    padding: theme.spacing(1, 1.5),
    textAlign: "left",
  },
  "& th": {
    backgroundColor: theme.palette.action.hover,
    fontWeight: theme.typography.fontWeightBold,
  },
  "& tr:nth-of-type(even)": {
    backgroundColor: theme.palette.action.hover, // 隔行变色
  },

  // ---- 其余保持原样 ----
  "& code": {
    ...theme.typography.body2,
    backgroundColor: theme.palette.action.selected,
    padding: "2px 4px",
    borderRadius: 4,
  },
}));

type AppMarkDownProps = {
  children: string;
  handlePathUrl?: (href: string) => void;
  placeHolder?: string;
  appcodeExtra?: AppCodeExtraUI;
  appCodeHandleEdit?: AppCodeHanleEdit;
  highlightStr?: string;
};

const rehypeHighlightWords = ({ highligth }) => {
  return (tree) => {
    visit(tree, "text", (node, index, parent) => {
      // 1. Basic guard clauses: if these match, we do nothing and move on
      if (!node.value || !parent || parent.tagName === "code") return;

      if (!highligth) return;

      // 2. Logic to decide if we SHOULD skip (e.g., if it doesn't contain a specific string)
      if (!node.value.toLowerCase().includes(highligth)) {
        return; // <--- This returns the ORIGINAL node by doing nothing
      }

      // console.log("Matching node found: ", node.value);

      // 3. Construct the HAST node (Only runs if the check above passes)
      const newNode = {
        type: "element",
        tagName: "mark",
        properties: {
          style: "background-color: yellow; color: black;",
        },
        children: [{ type: "text", value: node.value }],
      };

      // 4. Perform the replacement
      parent.children.splice(index, 1, newNode);

      // 5. Skip the newly created node to prevent recursion
      return [SKIP, index + 1];
    });
  };
};

function AppMarkDown(props: AppMarkDownProps) {
  let {
    children,
    handlePathUrl,
    placeHolder,
    appcodeExtra,
    appCodeHandleEdit,
    highlightStr,
  } = props;
  if (highlightStr) {
    highlightStr = highlightStr.toLowerCase();
  }
  function CustomLink(props: { href: string; children: ReactNode }) {
    const { href, children } = props;
    const { showNotification } = useNotification();
    const handleClick = (event) => {
      event.preventDefault();
      // 在这里处理链接点击，比如使用 Tauri API 打开链接
      console.info(`Link clicked: ${href}`);
      if (href.startsWith("http")) {
        openUrl(href)
          .then(() => {
            showNotification({ severity: "success", title: "open url OK" });
          })
          .catch((e) => {
            showNotification({
              severity: "error",
              title: "open url failed",
              message: e,
            });
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
      <div style={{ padding: "2px 1rem" }}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[[rehypeHighlightWords, { highligth: highlightStr }]]}
          components={{
            code({ className, children, ...props }) {
              const match = /language-(\w+)/.exec(className || "");
              return match ? (
                <AppCode
                  highlightStr={highlightStr}
                  extra={appcodeExtra}
                  language={match[1]}
                  handleEdit={appCodeHandleEdit}
                  codeBlock={String(children).replace(/\n$/, "")}
                  style={{ maxHeight: "40vh" }}
                />
              ) : (
                <code className={className} {...props}>
                  {children}
                </code>
              );
            },
            a: CustomLink,
          }}>
          {children ? children : placeHolder}
        </ReactMarkdown>
      </div>
    </MarkdownBody>
  );
}

function UrlDocument(props: { url: string }) {
  const { url } = props;
  return (
    <Tooltip title={url}>
      <InfoIcon
        href={url}
        style={{ cursor: "pointer" }}
        onClick={(e) => {
          e.preventDefault();
          openUrl(url);
        }}
      />
    </Tooltip>
  );
}

function SaveFavoriteSqlViewOnTheFly(props: {
  dbName: string;
  kind: DataSourceKind;
  sql: string;
  onChangeFavName: (name: string) => void;
  handleClose: () => void;
}) {
  const { kind, sql, onChangeFavName, dbName, handleClose } = props;
  const [favoriteName, setFavoriteName] = useState("");
  const { showNotification } = useNotification();
  const { save } = useFavoriteStore();
  useEffect(() => {
    onChangeFavName(favoriteName);
  }, [favoriteName, onChangeFavName]);

  return (
    <Stack>
      <TextField
        style={{ width: "15rem" }}
        label="name"
        value={favoriteName}
        onChange={(e) => {
          setFavoriteName(e.target.value);
        }}
        required
      />
      <Button
        onClick={() => {
          try {
            if (!favoriteName) {
              showNotification({
                severity: "warning",
                title: "please input favorite name",
              });

              return;
            }
            save({ dbName, name: favoriteName, kind, sql });
            showNotification({
              severity: "success",
              title: "save favorite sql success.",
            });
            handleClose();
          } catch (e) {
            showNotification({
              severity: "error",
              title: "save favorite sql failed",
              message: e,
            });
          }
        }}>
        ok
      </Button>
    </Stack>
  );
}

function SaveFavoriteSqlView(props: {
  favoriteName?: string;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { dbName, kind } = useContext(DbnameContext);

  const { favoriteName: oldName, editor } = props;
  const [favoriteName, setFavoriteName] = useState(oldName);
  const { showNotification } = useNotification();
  const { setOneShotUI } = useOneShotDialogUI();
  const { save } = useFavoriteStore();
  return (
    <>
      <Tooltip
        title={`save favorite sql ${favoriteName && `as named '${favoriteName}'`}`}>
        <IconButton
          onClick={async () => {
            try {
              const sql = editor.getValue();
              if (!sql) {
                showNotification({
                  severity: "warning",
                  title: "can't save empty statement.",
                });
                return;
              }
              if (favoriteName) {
                save({ dbName, name: favoriteName, kind, sql });
                showNotification({
                  severity: "success",
                  title: "save favorite sql success.",
                });
              } else {
                setOneShotUI({
                  ui: (
                    <SaveFavoriteSqlViewOnTheFly
                      dbName={dbName}
                      sql={sql}
                      kind={kind}
                      onChangeFavName={(name) => {
                        setFavoriteName(name);
                      }}
                      handleClose={() => {
                        setOneShotUI({ ui: null });
                      }}
                    />
                  ),
                  title: "set favorite name",
                });
              }
            } catch (e) {
              showNotification({
                severity: "error",
                title: "save failed.",
                message: e,
              });
            }
          }}
          color="primary">
          <FaBookmark style={{ fontSize: "1.2rem" }} />
        </IconButton>
      </Tooltip>
    </>
  );
}
/**
 * Cancel button used for ongoing operations.
 */
function CancelButton(props: { handleCancel: () => void }) {
  const { handleCancel } = props;
  return (
    <IconButton
      color="warning"
      onClick={() => {
        handleCancel();
      }}>
      <CancelIcon /> <CircularProgress />
    </IconButton>
  );
}

function SliderSize(props: { config: UseSizeConfig }) {
  const { config } = props;
  const width = "33vw";
  const { sizes, setSize } = useSizeStore();
  const size = sizes[config.key];

  const sizeIsNumber = typeof size === "number";
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
        }}>
        <Stack>
          {!sizeIsNumber && (
            <Typography color="warning">
              please select a size or default value is given
            </Typography>
          )}
          <Slider
            value={size}
            min={config.min}
            max={config.max}
            onChange={(_, v) => {
              setSize(config.key, v);
            }}
            color="success"
            step={config.step}
          />
          {config.notice}
        </Stack>
      </Paper>>
      <Chip
        style={{ ...textEllipsisStyle }}
        label={`${sizeIsNumber ? size : config.default}`}
        color="info"
      />
    </Tooltip>
  );
}

// select column include asc or desc
// useful for create index
/**
 * Multi-column selector with optional ASC ordering.
 */
function SelectColumnAsc(props: {
  fields: string[];
  includes: CreateIndexColumn[];
  setIncludes: (includes: CreateIndexColumn[]) => void;
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
              key={f.col}>
              <Typography color="info" style={{ width: "55%" }}>
                {f.col}
              </Typography>
              {supportOrder && (
                <FormControlLabel
                  label="asc"
                  control={
                    <Checkbox
                      checked={f.asc}
                      onChange={(e) => {
                        includes[index].asc = e.target.checked;
                        setIncludes([...includes]);
                      }}
                    />
                  }
                />
              )}
              <IconButton
                onClick={() => {
                  let i = includes.filter((x) => x.col != f.col);
                  setIncludes(i);
                }}>
                <CancelIcon> </CancelIcon>
              </IconButton>
            </Stack>
          );
        })}
      </Stack>
      <Autocomplete
        style={{ flex: 1 }}
        options={fields.filter((e) => {
          return !includes.find((f) => f.col == e);
        })}
        freeSolo={false}
        value={value}
        onChange={(_, value) => {
          setValue(value);
          if (value) {
            const index = includes.findIndex((e) => e.col == value);
            if (index == -1) {
              setIncludes([...includes, { col: value, asc: null }]);
            }
          }
        }}
        renderInput={(params) => <TextField {...params} label="select columns" />}
      />
    </Stack>
  );
}

function CreateIndexView(props: {
  fields: string[];
  handleReq: (x: CreateIndexReq) => void;
  kind: DataSourceKind;
  style?: CSSProperties;
}) {
  const { handleReq, kind, fields, style } = props;
  const indexTypes = ["index", "unique"];
  const [req, setReq] = useState<CreateIndexReq>({
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
            onChange={(e) => {
              setReq({ ...req, kind: e.target.value as IndexKind });
            }}
            value={req.kind}>
            {indexTypes.map((e) => (
              <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
            ))}
          </RadioGroup>
        </>
      );
      break;
  }
  return (
    <Stack style={style}>
      <ClearableTextField
        value={req.name}
        onChange={(e) => {
          setReq({ ...req, name: e.target.value });
        }}
        label="index name"
        required
        onClear={() => setReq({ ...req, name: "" })}
      />
      <SelectColumnAsc
        includes={req.columns}
        setIncludes={(rs) => {
          setReq({ ...req, columns: rs });
        }}
        kind={kind}
        fields={fields}
      />
      {indexTypeJsx}
    </Stack>
  );
}
/**
 * Autocomplete input for selecting a SQL type.
 */
function TypeSelect(props: {
  handleChange: (value: string | null) => void;

  type: string | null;
  style?: CSSProperties;
}) {
  const { dbName } = useContext(DbnameContext);
  const { handleChange, type, style } = props;
  const [types, setTypes] = useState<string[]>([]);
  const { showNotification } = useNotification();
  useEffect(() => {
    commands
      .listDataTypes(dbName)
      .then((data) => {
        setTypes(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "should not return a error",
          message: e,
        });
      });
  }, [showNotification, dbName]);
  return (
    <Autocomplete
      freeSolo
      options={types}
      style={{ width: "22rem", ...style }}
      value={type}
      renderInput={(params) => (
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
  nullable: SqlNullAble;
  handleNullable: (_: SqlNullAble) => void;
}) {
  const { nullable, handleNullable: handleNullness } = props;
  const opts: SqlNullAble[] = ["null", "notnull", "unspecified"];
  return (
    <Autocomplete
      value={nullable}
      options={opts}
      style={{ flexGrow: 1, flexShrink: 2, minWidth: "3rem" }}
      renderInput={(params) => <TextField {...params} label="specify null" />}
      onChange={(_, value) => {
        handleNullness(value);
      }}
      freeSolo={false}
    />
  );
}

function SqlDefaultValue(props: {
  defaultValue: string | null;
  setDefaultValue: (value: string | null) => void;
}) {
  const { dbName } = useContext(DbnameContext);
  const { defaultValue, setDefaultValue } = props;
  const [currentValue, setCurrentValue] = useState<string[]>([]);
  const { showNotification } = useNotification();
  useEffect(() => {
    commands
      .listSqlCurrentValues(dbName)
      .then((data) => {
        setCurrentValue(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list created types failed",
          message: e,
        });
      });
  }, [showNotification, dbName]);

  return (
    <Autocomplete
      freeSolo
      options={currentValue}
      style={{ flexGrow: 1, flexShrink: 2, minWidth: "3rem" }}
      value={defaultValue}
      renderInput={(params) => <TextField {...params} label="default value" />}
      onChange={(_, value) => {
        setDefaultValue(value);
      }}
      onInputChange={(_, value) => {
        setDefaultValue(value);
      }}
    />
  );
}

function AppDataGridPagination(props: {
  showPageSelect?: boolean;
  disabled?: boolean;
}) {
  const { showPageSelect, disabled } = props;
  const apiRef = useGridApiContext();
  const paginationModel = useGridSelector(apiRef, gridPaginationModelSelector);
  const pageCount = useGridSelector(apiRef, gridPageCountSelector);
  return (
    <>
      <Pagination
        count={pageCount}
        page={paginationModel.page + 1}
        color="primary"
        disabled={disabled}
        onChange={(_, value) => apiRef.current.setPage(value - 1)}
      />
      {showPageSelect && (
        <SelectPageSize
          pageSize={apiRef.current.state.pagination.paginationModel.pageSize}
          handleChange={(pageSize) => {
            apiRef.current.setPageSize(pageSize);
          }}
        />
      )}
    </>
  );
}

function AppDataGridToolBar(props: {
  titleName: string;
  left?: ReactNode;
  right?: ReactNode;
}) {
  const { titleName, left, right } = props;
  return (
    <Stack
      direction="row"
      style={{
        alignItems: "center",
        justifyContent: "space-between",
        border: "2px solid gray",
        borderBottom: "0.5rem solid gray",
        borderRadius: "4px",
        padding: "2px 4px",
      }}>
      <DataGridPageKeyboardControl />
      <StackRowCenter>{left}</StackRowCenter>
      {titleName && <Typography variant="h5">{titleName}</Typography>}
      <Stack direction="row" style={{ alignItems: "center" }}>
        {right} <CustomToolbar />
      </Stack>
    </Stack>
  );
}

/**
 * Displays a running query status indicator.
 */
function QueryStatus(props: { status?: string }) {
  const { status } = props;
  return (
    status && (
      <StackRowCenter>
        <CircularProgress />
        <Typography> {status} </Typography>
      </StackRowCenter>
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

/**
 * Modal used to set a saved filter or sort expression for a table.
 */
function SetFilterSort(props: {
  open: boolean;
  handleClose: () => void;
  value: string;
  setValue: (value: string) => void;
  label: "filter" | "sort";
  tableName: string;
  kind: DataSourceKind;
}) {
  const { dbName } = useContext(DbnameContext);
  let { open, handleClose, value, setValue, label, tableName, kind } = props;
  const isFilter = label == "filter";
  const extractFn = isFilter
    ? function (sql: string) {
        return commands.extractTableSelectSelection(dbName, sql);
      }
    : function (sql: string) {
        return commands.extractTableSelectSort(dbName, sql);
      };
  tableName = quoteDatabaseIdentifier(tableName, kind);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [modalBox, setModalBox] = useState(null);
  const { showNotification } = useNotification();

  const [selectFilter, setSelectFilter] = useState("");
  const { getFilter, addFilter, removeFilter } = useTableFilter();
  const filters = getFilter(dbName, tableName);
  const tabSize = useTabSize();
  const handleSet = async () => {
    try {
      let data = await extractFn(editor.getValue());
      setValue(data == null ? "" : data);
      handleClose();
    } catch (e) {
      showNotification({
        severity: "error",
        title: "extract sql error.",
        message: e,
      });
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
      disableAutoFocus
      disableEnforceFocus>
      <Box
        sx={{ resize: "both", ...modalStyle, width: "75%", height: "30vh" }}
        ref={setModalBox}>
        <ModalHeader title={`set ${label}`} handleClose={handleClose} />
        <SimpleFlexEditor
          value={
            formatSqlFn(
              kind,
              isFilter
                ? `SELECT * FROM ${tableName} WHERE ${value}`
                : `SELECT * FROM ${tableName} ORDER BY ${value}`,
              tabSize,
            ) + " "
          }
          handleEditor={(e) => {
            setEditor(e);
            e.focus();
            editorPositionToEnd(e);
            e.getModel()._dbpal_query_id = new NameAndQueryId(dbName, kind);
          }}
          language={"sql"}
        />

        <ListenElementSize name="where-sort" dom={modalBox} set="both" />

        <StackRowCenter>
          <Button onClick={handleSet}>set {label}</Button>
          {isFilter && (
            <>
              <Tooltip title="save current value to filter bookmark">
                <IconButton
                  onClick={async () => {
                    const value = editor.getValue();
                    if (!value) {
                      showNotification({
                        severity: "warning",
                        title: "this editor is empty",
                      });
                      return;
                    }
                    try {
                      const data = await extractFn(value);
                      addFilter(dbName, tableName, data);
                    } catch (e) {
                      showNotification({
                        severity: "error",
                        title: "save filter failed",
                        message: e,
                      });
                    }
                  }}>
                  <SaveIcon />
                </IconButton>
              </Tooltip>
              <IconButton
                onClick={() => {
                  removeFilter(dbName, tableName);
                  showNotification({
                    severity: "success",
                    title: "delete filter success",
                  });
                }}
                disabled={filters.length == 0}>
                <DeleteForeverIcon />
              </IconButton>
              <Autocomplete
                value={selectFilter}
                style={{ flex: 1 }}
                options={filters}
                freeSolo={false}
                onChange={async (_, value) => {
                  editor.setValue(
                    formatSqlFn(
                      kind,
                      `SELECT * FROM ${tableName} WHERE ${value}`,
                      tabSize,
                    ),
                  );
                  setSelectFilter(value);
                }}
                renderInput={(params) => <TextField {...params} label="filters" />}
              />
            </>
          )}
        </StackRowCenter>
      </Box>
    </Modal>
  );
}

function ToggleChatShow() {
  const { show, setShow } = useContext(ChatShowContext);

  const llm = useLLM();
  return (
    llm && (
      <>
        <IconButton
          onClick={() => {
            setShow(!show);
          }}>
          {show ? (
            <ChatBubbleIcon color="success" />
          ) : (
            <ChatBubbleOutlineOutlinedIcon />
          )}
        </IconButton>
      </>
    )
  );
}

/**
 * Copy SQL button with inline SQL preview.
 */
function CopySql(props: { sql: string; style?: CSSProperties }) {
  let { sql, style } = props;
  const { showNotification } = useNotification();
  if (!sql) {
    return <> </>;
  }
  return (
    <Stack style={{ ...style, alignItems: "center" }} direction="row">
      <IconButton
        onClick={() => {
          handleCopyToClipboard(sql, showNotification);
        }}>
        <ContentCopyIcon />
      </IconButton>
      <code> {sql} </code>
    </Stack>
  );
}

function IconModifed() {
  return <CircleIcon color="warning" style={{ fontSize: "0.9rem" }} />;
}

function ListenElementSize(props: {
  dom?: Element;
  name: string;
  set: "height" | "width" | "both";
}) {
  const { elements, setElement } = useElementSizeStore();
  const { dom, name, set } = props;

  useEffect(() => {
    if (!dom) {
      return;
    }
    const e = new ResizeObserver((en) => {
      const height = en[0].target.clientHeight;
      const width = en[0].target.clientWidth;
      if (width && height) {
        setElement(name, { width, height });
      }
    });

    e.observe(dom);
    return () => {
      e.disconnect();
    };
  }, [dom, name, setElement]);

  useEffect(() => {
    if (!dom) {
      return;
    }
    const size = elements[name];
    if (size) {
      const div = dom as HTMLDivElement;
      if (set == "height" || set == "both") {
        if (size.height) {
          // eslint-disable-next-line react-hooks/immutability
          div.style.height = size.height + "px";
        }
      }
      if (set == "width" || set == "both") {
        if (size.width) {
          div.style.width = size.width + "px";
        }
      }
    }

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dom]);

  return <> </>;
}

function AiMarkdownModel(props: {
  editor: editor.IStandaloneCodeEditor;
  markdownModel: boolean;
  appcodeExtra?: AppCodeExtraUI;
  appCodeHandleEdit?: AppCodeHanleEdit;
}) {
  const { editor, markdownModel, appcodeExtra, appCodeHandleEdit } = props;
  const markdownDivDom = useRef<HTMLDivElement>(null);
  const [markdownModelValue, setMarkdownModelValue] = useState("");
  useEffect(() => {
    if (!editor) {
      return;
    }
    if (!markdownModel) {
      return;
    }
    setMarkdownModelValue(editor.getValue());
    let d1 = editor.onDidChangeModelContent(() => {
      setMarkdownModelValue(editor.getValue());
      // 1. 计算距离底部的剩余距离
      // scrollHeight (内容总高) - scrollTop (已滚距离) - clientHeight (可视区高度)
      let dom = markdownDivDom.current;
      const distanceFromBottom = dom.scrollHeight - dom.scrollTop - dom.clientHeight;
      // 2. 设置一个缓冲区阈值（如 100 像素）
      // 如果距离底部小于 100px，说明用户还在底部附近，允许自动滚动
      if (distanceFromBottom < 220) {
        dom.scrollTop = dom.scrollHeight;
      }
    });
    return () => {
      return d1.dispose();
    };
  }, [editor, markdownModel, setMarkdownModelValue]);

  if (!markdownModel) {
    return <> </>;
  }
  return (
    <div
      style={{
        flexGrow: 1,
        flexShrink: 1,
        flexBasis: 0,
        minHeight: 0,
        height: "100px",
        overflow: "auto",
      }}
      ref={markdownDivDom}>
      <AppMarkDown appCodeHandleEdit={appCodeHandleEdit} appcodeExtra={appcodeExtra}>
        {markdownModelValue}
      </AppMarkDown>
    </div>
  );
}

/**
 * Toggle switch for AI markdown editing mode.
 */
function AiMarkdownModelSwitch(props: {
  setMarkdownModel: Dispatch<SetStateAction<boolean>>;
  markdownModel: boolean;
}) {
  const { dbName } = useContext(DbnameContext);
  const { setMarkdownModel, markdownModel } = props;
  const { getSwitch, setSwitch } = useBoolSwitch();
  const switchKey = "ai-markdown-model" + "-" + dbName;
  const llm = useLLM();
  const vis = useContext(TabActiveContext);
  useEffect(() => {
    if (!llm) {
      return;
    }
    const v = getSwitch(switchKey);
    setMarkdownModel(v);
  }, [getSwitch, llm, setMarkdownModel, switchKey]);

  useEffect(() => {
    if (!llm) {
      return;
    }
    setSwitch(switchKey, markdownModel);
  }, [llm, markdownModel, setSwitch, switchKey]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (KeyPressed.editorMarkdown(e) && vis) {
        e.preventDefault();
        setMarkdownModel((e) => !e);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [setMarkdownModel, vis]);

  return (
    llm && (
      <Tooltip
        title={`toggle markdown model, keyboard shortcut is ${isMacOs() ? "⌘" : "Ctrl"} + B`}>
        <FormControlLabel
          control={
            <Switch
              onChange={(_e, checked) => {
                setMarkdownModel(checked);
              }}
              checked={markdownModel}
            />
          }
          label="markdown model"
        />
      </Tooltip>
    )
  );
}

function EscPressedDo(props: { doingIt: () => boolean; setUndo: () => void }) {
  const vis = useContext(TabActiveContext);
  const { doingIt, setUndo } = props;

  useEffect(() => {
    const closeThis = (e: KeyboardEvent) => {
      if (e.key === "Escape" && vis && doingIt()) {
        e.preventDefault();
        setUndo();
      }
    };
    document.addEventListener("keydown", closeThis);
    return () => {
      document.removeEventListener("keydown", closeThis);
    };
  }, [setUndo, doingIt, vis]);

  return <></>;
}

/**
 * Display count summary when filtered results are less than total.
 */
function ShowingCount(props: {
  showing: number;
  total: number;
  style?: CSSProperties;
}) {
  const { showing, total, style } = props;
  if (showing == total) {
    return <></>;
  }

  return (
    <Typography color="info" style={{ ...textEllipsisStyle, ...style }}>
      showing {showing} of {total}
    </Typography>
  );
}

/**
 * Shows fetch duration and completion time while loading.
 */
function FetchTimeAndLoadingStatus(props: { isLoading: boolean }) {
  const { isLoading } = props;
  const [fetchTime, setFetchTime] = useState<number | null>(null);
  const [date, setDate] = useState<Date>(null);
  const fetchBeginTime = useRef(new Date().getTime());
  const firstTime = useRef(true);

  useEffect(() => {
    if (isLoading) {
      firstTime.current = false;
      fetchBeginTime.current = new Date().getTime();
      const int = setInterval(() => {
        setFetchTime(new Date().getTime() - fetchBeginTime.current);
      }, 100);
      return () => {
        clearInterval(int);
      };
    } else if (!firstTime.current) {
      // means end the loading
      setFetchTime(new Date().getTime() - fetchBeginTime.current);
      setDate(new Date());
    }
  }, [isLoading]);

  return (
    <>
      {date && (
        <Tooltip title="fetch time">
          <Chip
            color="info"
            label={date.toLocaleTimeString()}
            icon={<IoTimerOutline />}
          />
        </Tooltip>
      )}
      {!!fetchTime && (
        <Tooltip title="fetch used duration">
          <Chip
            label={prettyMs(fetchTime)}
            color={fetchTime > 5000 ? "error" : "success"}
            icon={<MdTimer />}
          />
        </Tooltip>
      )}
    </>
  );
}

function FetchTimeDataGrid() {
  const apiRef = useGridApiContext();

  // 使用选择器获取当前的 loading 状态
  const isLoading: boolean = useGridSelector(apiRef, gridRowsLoadingSelector);

  return <FetchTimeAndLoadingStatus isLoading={isLoading} />;
}

function DataGridPageKeyboardControl() {
  const vis = useContext(TabActiveContext);
  const apiRef = useGridApiContext();
  const { showNotification } = useNotification();

  useEffect(() => {
    const callback = (e: KeyboardEvent) => {
      if (!vis) return;
      const paginationModel = apiRef.current.state.pagination.paginationModel;
      const pageCount = Math.ceil(
        apiRef.current.state.rows.totalRowCount / paginationModel.pageSize,
      );
      if (KeyPressed.nextPage(e)) {
        if (paginationModel.page < pageCount - 1) {
          apiRef.current.setPage(paginationModel.page + 1);
          e.preventDefault();
          showNotification({ severity: "success", title: "next page pressed" });
        }
      }
      if (KeyPressed.prevPage(e)) {
        if (paginationModel.page > 0) {
          e.preventDefault();
          showNotification({ severity: "success", title: "prev page pressed" });
          apiRef.current.setPage(paginationModel.page - 1);
        }
      }
    };
    document.addEventListener("keydown", callback);
    return () => {
      document.removeEventListener("keydown", callback);
    };
  }, [apiRef, showNotification, vis]);

  return <> </>;
}

function ShowQueryLog() {
  const [showQueryLog, setShowQueryLog] = useState(false);

  return (
    <>
      <Tooltip title="query log">
        <IconButton
          onClick={() => {
            setShowQueryLog((prev) => !prev);
          }}
          color="primary">
          <EventNoteIcon />
        </IconButton>
      </Tooltip>
      <QueryLog showQueryLog={showQueryLog} setShowQueryLog={setShowQueryLog} />
    </>
  );
}

function RefreshButton(props: IconButtonProps) {
  return (
    <IconButton {...props}>
      <LuRefreshCcw fontSize="1.3rem" />
    </IconButton>
  );
}

function ExportQueryResultToFile() {
  // Store the clicked HTML element instead of a boolean
  const [anchorEl, setAnchorEl] = useState(null);
  const openMenu = Boolean(anchorEl);
  const api = useGridApiContext();

  const { showNotification } = useNotification();

  const handleWriteFile = async (
    extension: string,
    exportFn: (_: (_: string) => Promise<void>) => void,
  ) => {
    let fileHandle: FileHandle;
    try {
      const path = await save({
        defaultPath: generateDefaultFilename("query_resp", extension),
      });
      if (!path) {
        return;
      }
      fileHandle = await openFile(path, { write: true, truncate: true, create: true });
      await exportFn(async (x: string) => {
        await fileHandle.write(new TextEncoder().encode(x));
      });
      showNotification({
        severity: "success",
        title: "download ok",
        message: <OpenFileWithDefaultApp file={path} />,
      });
    } catch (e) {
      showNotification({ severity: "error", message: e, title: "export failed" });
    } finally {
      if (fileHandle) {
        fileHandle.close();
      }
    }
  };

  return (
    <>
      <Tooltip title="download this result to a file">
        <IconButton
          onClick={(event) => {
            setAnchorEl(event.currentTarget);
          }}
          color="primary">
          <DownloadIcon />
        </IconButton>
      </Tooltip>

      {anchorEl && (
        <Menu
          open={openMenu}
          anchorEl={anchorEl} // 🚀 Tells Menu exactly where to position itself
          anchorReference="anchorEl"
          onClose={() => {
            setAnchorEl(null);
          }}
          onClick={() => {
            setAnchorEl(null);
          }}>
          <MenuList>
            <MenuItem
              onClick={() => {
                handleWriteFile("csv", async (fileHandle) => {
                  let fs = api.current.getAllColumns().map((x) => x.field);
                  await fileHandle(createCSV(fs));
                  for (const id of api.current.getSortedRowIds()) {
                    let values = api.current
                      .getAllColumns()
                      .map((f) => api.current.getCellValue(id, f.field));
                    await fileHandle(createCSV(values));
                  }
                });
              }}>
              <ListItemText> Download as CSV </ListItemText>
            </MenuItem>

            <MenuItem
              onClick={() => {
                handleWriteFile("json", async (fileHandle) => {
                  let first = true;
                  await fileHandle("[\n");
                  for (const id of api.current.getSortedRowIds()) {
                    let o = {};
                    for (const f of api.current.getAllColumns()) {
                      o[f.field] = parseCellType(api.current.getCellValue(id, f.field));
                    }
                    if (!first) {
                      await fileHandle(",\n");
                    }
                    if (first) {
                      first = false;
                    }
                    await fileHandle(JSON.stringify(o, null, 2));
                  }
                  await fileHandle("\n]");
                });
              }}>
              <ListItemText> Download as JSON </ListItemText>
            </MenuItem>

            <MenuItem
              onClick={async () => {
                const all: XlsxCellData[] = [];
                api.current.getAllColumns().forEach((f, index) => {
                  all.push({ row: 0, column: index, content: f.field });
                });
                for (const [row, id] of api.current.getSortedRowIds().entries()) {
                  for (const [column, f] of api.current.getAllColumns().entries()) {
                    const content = api.current.getCellValue(id, f.field);
                    if (content) {
                      all.push({ row: row + 1, column: column, content });
                    }
                  }
                }
                try {
                  const path = await save({
                    filters: [{ extensions: ["xlsx"], name: "Excel file" }],
                    defaultPath: generateDefaultFilename("export-xlsx", "xlsx"),
                  });
                  if (!path) {
                    return;
                  }
                  await commands.writeXlsx(path, all);
                  showNotification({
                    severity: "success",
                    message: <OpenFileWithDefaultApp file={path} />,
                    title: "export file ok",
                  });
                } catch (e) {
                  showNotification({
                    severity: "error",
                    message: e,
                    title: "write xlsx file failed",
                  });
                }
              }}>
              <ListItemText> Download as Excel </ListItemText>
            </MenuItem>
            <Divider />
            <MenuItem
              onClick={async () => {
                let result = "";
                const fs = api.current.getAllColumns().map((f) => f.field);
                result += createCSV(fs);
                for (const id of api.current.getSortedRowIds()) {
                  const values = api.current
                    .getAllColumns()
                    .map((f) => api.current.getCellValue(id, f.field));
                  result += createCSV(values);
                }
                handleCopyToClipboard(result, showNotification);
              }}>
              <ListItemText> Copy as CSV </ListItemText>
            </MenuItem>

            <MenuItem
              onClick={async () => {
                const result: string[][] = [];
                result.push(api.current.getAllColumns().map((f) => f.field));
                for (const id of api.current.getSortedRowIds()) {
                  const values = api.current
                    .getAllColumns()
                    .map((x) => api.current.getCellValue(id, x.field));

                  result.push(values);
                }
                handleCopyToClipboard(arrayToMarkdownTable(result), showNotification);
              }}>
              <ListItemText> Copy as Markdown </ListItemText>
            </MenuItem>

            <MenuItem
              onClick={async () => {
                const result = [];

                for (const id of api.current.getSortedRowIds()) {
                  let o = {};
                  for (const f of api.current.getAllColumns()) {
                    o[f.field] = parseCellType(api.current.getCellValue(id, f.field));
                  }
                  result.push(o);
                }

                handleCopyToClipboard(
                  JSON.stringify(result, null, 2),
                  showNotification,
                );
              }}>
              <ListItemText> Copy as JSON </ListItemText>
            </MenuItem>
          </MenuList>
        </Menu>
      )}
    </>
  );
}

function CopyAsXXX(props: {
  content: string;
  getRow: () => object;
  field: string;
  dbName: string;
  fieldType: string;
}) {
  const { content, getRow, field, dbName, fieldType } = props;

  const { showNotification } = useNotification();

  return (
    <>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <FaCopy fontSize="1.4rem" />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={() => {
          handleCopyToClipboard(field, showNotification);
        }}>
        <ListItemIcon>{/* <FaCopy fontSize="1.4rem" /> */}</ListItemIcon>
        <ListItemText> Copy Column Name </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={() => {
          const row = getRow();
          for (const x in row) {
            row[x] = parseCellType(row[x]);
          }
          handleCopyToClipboard(JSON.stringify(row, null, 2), showNotification);
        }}>
        <ListItemIcon>{/* <FaCopy fontSize="1.4rem" /> */}</ListItemIcon>
        <ListItemText> Copy as JSON </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={() => {
          const row = getRow();
          const result: string[][] = [];
          result.push(Object.keys(row));
          result.push(Object.keys(row).map((x) => row[x]));
          handleCopyToClipboard(arrayToMarkdownTable(result), showNotification);
        }}>
        <ListItemIcon>{/* <FaCopy fontSize="1.4rem" /> */}</ListItemIcon>
        <ListItemText> Copy as Markdown </ListItemText>
      </MenuItem>

      {content && (
        <MenuItem
          onClick={async () => {
            try {
              const value = await commands.putValueInSql(dbName, fieldType, content);
              handleCopyToClipboard(`(${value})`, showNotification);
            } catch (e) {
              showNotification({
                severity: "error",
                message: e,
                title: "format for in failed",
              });
            }
          }}>
          <ListItemIcon>{/* <FaCopy fontSize="1.4rem" /> */}</ListItemIcon>
          <ListItemText> Copy as For in </ListItemText>
        </MenuItem>
      )}
    </>
  );
}

function SqlTableViewData(props: {
  handleData: (args: any) => Promise<void>;
  fields: Field[];
  initValues?: object;
  buttonName: string;
  isAdd: boolean;
  handleClose?: () => void;
  extraButtons?: [{ name: string; handleValue: (value: object) => void }];
}) {
  let { handleData, fields, initValues, buttonName, isAdd, handleClose, extraButtons } =
    props;
  if (!initValues) {
    initValues = {};
  }
  fields.forEach((e) => {
    if (e.is_auto_increment && isAdd) {
      initValues[e.field] = "";
    }
  });
  const { filter, setFilter } = useSqlDataFilter();

  const filterFn = useMemo(() => {
    return filter
      ? wcmatch("*" + filter + "*", { flags: "i" })
      : () => {
          return true;
        };
  }, [filter]);

  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState(initValues);
  const { showNotification } = useNotification();
  const inputDoms = useRef({});

  const handleModify = () => {
    const argsclone = { ...args };
    for (let i = 0; i < fields.length; i++) {
      const f = fields[i];
      if (f.is_auto_increment) {
        continue;
      }
      if (!argsclone[f.field] && !f.nullable && !f.default_value) {
        inputDoms.current[f.field].focus();
        showNotification({
          severity: "error",
          title: `filed '${f.field}' need be filled`,
        });
        return;
      }
    }
    for (const key in argsclone) {
      if (!argsclone[key]) {
        // make sure '' convert to null
        argsclone[key] = null;
      }
    }
    setLoading(true);
    handleData(argsclone)
      .then(() => {
        showNotification({ severity: "success", title: `${buttonName} OK` });
        if (handleClose) {
          handleClose();
        }
      })
      .catch((e) => {
        console.error("handle value failure", e);
        showNotification({
          severity: "error",
          title: `${buttonName} failure`,
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      {fields
        .filter(
          (f) =>
            filterFn(f.field) || (args[f.field] ? filterFn(args[f.field]) : !filter),
        )
        .map((f) => {
          return (
            <Stack
              key={f.field}
              direction="row"
              sx={{ alignContent: "center", justifyContent: "center" }}>
              <TextField
                inputRef={(x) => {
                  inputDoms.current[f.field] = x;
                }}
                disabled={f.is_auto_increment}
                value={args[f.field]}
                helperText={
                  f.data_type +
                  " " +
                  (f.nullable ? "null" : "not null") +
                  " " +
                  (f.is_auto_increment ? "auto increment" : "") +
                  " " +
                  (f.default_value ? `default ${f.default_value}` : "")
                }
                required={!f.nullable}
                label={f.field}
                multiline={isTypeLargeText(f.data_type)}
                rows={4}
                fullWidth
                onChange={(e) => {
                  const value = e.target.value;
                  const newArgs = { ...args };
                  newArgs[f.field] = value;
                  setArgs(newArgs);
                }}
              />
            </Stack>
          );
        })}
      <Divider />
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <Button loading={loading} onClick={handleModify}>
          {buttonName}
        </Button>
        {extraButtons &&
          extraButtons.map((callback) => (
            <Button
              key={callback.name}
              onClick={() => {
                const argsClone = { ...args };
                for (const key in argsClone) {
                  if (!argsClone[key]) {
                    argsClone[key] = null;
                  }
                }
                callback.handleValue(argsClone);
              }}>
              {callback.name}
            </Button>
          ))}
        <ClearableTextField
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
          label="filter"
          style={{ flex: 1 }}
          helperText="can match field name and content"
          onClear={() => {
            setFilter("");
          }}
        />
      </Stack>
    </Stack>
  );
}
export {
  SqlTableViewData,
  CopyAsXXX,
  ExportQueryResultToFile,
  ShowQueryLog,
  FetchTimeDataGrid,
  FetchTimeAndLoadingStatus,
  ShowingCount,
  CreateDatabase,
  CreateSchema,
  SimpleFlexEditor,
  MenuConnectionInfo,
  ConnConfigInfo,
  LoadStoreFile,
  AppMarkDown,
  UrlDocument,
  SaveFavoriteSqlView,
  CancelButton,
  SliderSize,
  SelectColumnAsc,
  CreateIndexView,
  TypeSelect,
  ColumnNullness,
  SqlDefaultValue,
  AppDataGridPagination,
  AppDataGridToolBar,
  QueryStatus,
  StackRowCenter,
  SetFilterSort,
  ToggleChatShow,
  CopySql,
  IconModifed,
  ListenElementSize,
  AppCode,
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  EscPressedDo,
  DataGridPageKeyboardControl,
  RefreshButton,
};
