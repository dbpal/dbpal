import {
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ClearIcon from "@mui/icons-material/Clear";
import Stack from "@mui/material/Stack";
import InfoIcon from "@mui/icons-material/Info";
import CloseIcon from "@mui/icons-material/Close";
import SaveIcon from "@mui/icons-material/Save";
import { FaBookmark } from "react-icons/fa";
import { IoMdInformationCircleOutline } from "react-icons/io";
import CancelIcon from "@mui/icons-material/Cancel";
import ChatBubbleOutlineOutlinedIcon from "@mui/icons-material/ChatBubbleOutlineOutlined";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import CircleIcon from "@mui/icons-material/Circle";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
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
  Switch,
  TextField,
  TextFieldProps,
  Tooltip,
  Typography,
} from "@mui/material";
import AttachFileIcon from "@mui/icons-material/AttachFile";
import {
  AppCodeExtra,
  AppCodeHanleEdit,
  ChatShowContext,
  DataSourceKind,
  editorPositionToEnd,
  editorSetPositionAndReveal,
  handleCopyToClipboard,
  isMacOs,
  KeyPressed,
  prettyMs,
  quoteDatabaseIdentifier,
  secondConfirm,
  TabActiveContext,
  TabContent,
  textEllipsisStyle,
} from "./helper";

import { editor, Position } from "monaco-editor";
import {
  dbpalQuerykey,
  EscDownEvent,
  extractBaseName,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  reloadOK,
} from "./Common";
import AddIcon from "@mui/icons-material/Add";
import ChatBubbleIcon from "@mui/icons-material/ChatBubble";
import { MetaValueView, ModalHeader, SelectPageSize } from "./CommonComp";
import { readFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { save, open, DialogFilter, message } from "@tauri-apps/plugin-dialog";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { styled, useTheme } from "@mui/material/styles";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  gridPageCountSelector,
  gridPaginationModelSelector,
  gridRowsLoadingSelector,
  useGridApiContext,
  useGridSelector,
} from "@mui/x-data-grid";
import { CustomToolbar } from "./CustomToolBar";
import {
  useBoolSwitch,
  useConnectionStore,
  useElementSizeStore,
  useFavoriteStore,
  useLLM,
  useNotification,
  UseSizeConfig,
  useSizeStore,
  useTableFilter,
  useTabsManage,
  useUserSettings,
} from "./store";
import { MonacoEditor } from "./Editor";
import EditIcon from "@mui/icons-material/Edit";
import { Highlight, themes } from "prism-react-renderer";

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
          endAdornment: value ? (
            <InputAdornment position="end">
              <IconButton onClick={onClear} edge="end">
                <ClearIcon fontSize="small" />
              </IconButton>
            </InputAdornment>
          ) : null,
        },
      }}
    />
  );
}

function AppCode(props: {
  codeBlock: string;
  language: string;
  extra?: AppCodeExtra;
  handleEdit?: AppCodeHanleEdit;
  highlightStr?: string;
}) {
  const theme = useTheme();
  const { codeBlock, language, extra, handleEdit, highlightStr } = props;
  const { showNotification } = useNotification();
  return (
    <Paper
      style={{
        padding: "2px 4px",
        borderRadius: "0.4rem",
        margin: 0,
        maxHeight: "55vh",
        overflow: "auto",
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

function CreateDatabase(props: {
  uuid: string;
  handleClose: () => void;
  kind: DataSourceKind;
}) {
  const { uuid, handleClose, kind } = props;
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
      .createDatabase(uuid, database, extra)
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

function CreateSchema(props: {
  uuid;
  handleClose;
  handleReload;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { uuid, handleClose, kind, schemas, handleReload } = props;
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
      .createSchema(uuid, schema, extra)
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

function SimpleFlexEditor(props: {
  language: string;
  value?: string;
  barLeft?: ReactNode;
  readOnly?: boolean;
  errPos?: Position;
  disableEditorBar?: boolean;
  handleEditor?: (editor: editor.IStandaloneCodeEditor) => void;
  handleValue?: (x) => void;
}) {
  const {
    language,
    value,
    barLeft,
    readOnly,
    errPos,
    handleEditor,
    disableEditorBar,
    handleValue,
  } = props;
  const [pos, setPos] = useState<Position>(null);
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
    const d1 = editor.onDidChangeCursorPosition(() => {
      setPos(editor.getPosition());
    });
    const d2 = editor.onDidChangeModelContent(() => {});
    return () => {
      d1.dispose();
      d2.dispose();
    };
  }, [editor, handleEditor]);
  useEffect(() => {
    if (editor && errPos) {
      editorSetPositionAndReveal(editor, errPos);
    }
  }, [editor, errPos]);

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
        />
      </div>
      {!disableEditorBar && <EditorBar left={barLeft} pos={pos} />}
    </div>
  );
}

function EditorBar(props: { left?: any; pos: Position }) {
  const { left, pos } = props;
  return (
    <Stack
      style={{ justifyContent: "space-between", alignItems: "center" }}
      direction="row">
      {left ? left : <span />}

      <Stack direction="row" style={{ paddingLeft: "2rem", paddingRight: "3rem" }}>
        <span>Ln {pos ? pos.lineNumber : ""}</span>
        <span>Col {pos ? pos.column : ""}</span>
      </Stack>
    </Stack>
  );
}

function MenuConnectionInfo({ uuid }) {
  const appendTab = useTabsManage((state) => state.appendTab);
  const handleInfo = async () => {
    appendTab(
      new TabContent(
        uuid,
        <ConnConfigInfo uuid={uuid} />,
        <IoMdInformationCircleOutline />,
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

function ConnConfigInfo(props: { uuid }) {
  const { uuid } = props;
  const { showNotification } = useNotification();
  const [metas, setMetas] = useState<MetaValue[]>([]);
  const { getConnection } = useConnectionStore();

  const handleInfo = useCallback(
    async (showOK: boolean) => {
      try {
        let metas: MetaValue[] = await commands.serverMetas(uuid);
        const config = await getConnection(uuid);
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
              { name: "host", value: config.host } as MetaValue,
              { name: "port", value: config.port + "" } as MetaValue,
              { name: "user", value: config.username } as MetaValue,
            ];
            break;
          case DataSourceKind.LibSql:
            if (config.source === "local") {
              configMetas = [{ name: "host", value: config.path } as MetaValue];
            } else if (config.source == "remote") {
              configMetas = [
                { name: "url", value: config.url } as MetaValue,
                { name: "path", value: config.path } as MetaValue,
              ];
            }
            break;
          case DataSourceKind.Sqlite:
            configMetas = [{ name: "path", value: config.path } as MetaValue];
            break;
          case DataSourceKind.CouchBase:
            configMetas = [
              { name: "username", value: config.username } as MetaValue,
              { name: "conn str", value: config.conn_str } as MetaValue,
            ];
            break;
        }
        setMetas([...configMetas, ...metas]);
        if (showOK) {
          reloadOK(showNotification);
        }
      } catch (e) {
        showNotification({
          severity: "error",
          title: "call server metas failed",
          message: e,
        });
      }
    },
    [getConnection, showNotification, uuid],
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
          }}>
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
        handleLoadFile(file, content);
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
        handleLoadFile(ofile, content);
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
      handleLoadFile("", "");
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
  appcodeExtra?: AppCodeExtra;
  appCodeHandleEdit?: AppCodeHanleEdit;
  highlightStr?: string;
};
import { visit, SKIP } from "unist-util-visit";
import {
  commands,
  CouchBaseBucketType,
  CreateDatabaseExtra,
  CreateIndexColumn,
  CreateIndexReq,
  IndexKind,
  MetaValue,
  SqlNullAble,
} from "./bindings";
import { IoTimerOutline } from "react-icons/io5";

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
  const { showNotification } = useNotification();

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
                save({ uuid, name: favoriteName, kind, sql, language });
              } else {
                setShowFavoriteName(true);
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

      {showSetFavoriteName && (
        <Modal
          open={showSetFavoriteName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFavoriteName(false);
          })}>
          <Box sx={{ ...modalStyle, width: "40%" }}>
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
                onChange={(e) => {
                  setFavoriteName(e.target.value);
                }}
                required
              />
              <Stack direction="row">
                <IconButton
                  color="primary"
                  onClick={() => {
                    if (!favoriteName) {
                      showNotification({
                        severity: "warning",
                        title: "name can't be empty",
                      });
                      return;
                    }
                    setShowFavoriteName(false);
                    save({
                      uuid,
                      name: favoriteName,
                      kind,
                      sql: editor.getValue(),
                      language,
                    });
                  }}>
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
        label={`${config.name ? config.name + ":" : ""} ${sizeIsNumber ? size : config.default}`}
        color="info"
      />
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
  includes: CreateIndexColumn[];
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
              setIncludes([
                ...includes,
                { col: value, asc: null } as CreateIndexColumn,
              ]);
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
function TypeSelect(props: { handleChange; uuid; type; style?: CSSProperties }) {
  const { handleChange, uuid, type, style } = props;
  const [types, setTypes] = useState<string[]>([]);
  const { showNotification } = useNotification();
  useEffect(() => {
    commands
      .listDataTypes(uuid)
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
  }, [showNotification, uuid]);
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
  const opts: SqlNullAble[] = ["null", "notnull", "none"];
  return (
    <Autocomplete
      value={nullable}
      options={opts}
      style={{ width: "7rem" }}
      renderInput={(params) => <TextField {...params} label="specify null" />}
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
  const { showNotification } = useNotification();
  useEffect(() => {
    commands
      .listSqlCurrentValues(uuid)
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
  }, [showNotification, uuid]);

  return (
    <Stack direction="row">
      <Autocomplete
        freeSolo
        options={currentValue}
        style={{ width: "14rem" }}
        value={defaultValue}
        renderInput={(params) => <TextField {...params} label="default value" />}
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

function LastFetchTime(props: { date: Date }) {
  const { date } = props;
  return <Typography color="info"> {date.toLocaleTimeString()} </Typography>;
}

function AppDataGridToolBar(props: {
  title?: string;
  left?: ReactNode;
  right?: ReactNode;
  disableBorder?: boolean;
}) {
  const { title, left, right, disableBorder } = props;
  const theme = useTheme();
  return (
    <Stack
      direction="row"
      style={{
        alignItems: "center",
        justifyContent: "space-between",
        border: disableBorder ? undefined : "2px solid " + theme.palette.secondary.main,
        borderRadius: "4px",
        padding: "2px 4px",
      }}>
      <StackRowCenter>{left}</StackRowCenter>
      {title && <Typography variant="h5">{title}</Typography>}
      <Stack direction="row" style={{ alignItems: "center" }}>
        {right}
        <CustomToolbar />
      </Stack>
    </Stack>
  );
}

function QueryStatus({ status }) {
  return (
    status && (
      <Stack direction="row">
        <CircularProgress />
        <Typography> {status} </Typography>
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
    ? function (uuid, sql) {
        return commands.extractTableSelectSelection(uuid, sql);
      }
    : function (uuid, sql) {
        return commands.extractTableSelectSort(uuid, sql);
      };
  tableName = quoteDatabaseIdentifier(kind)(tableName);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [modalBox, setModalBox] = useState(null);
  const { showNotification } = useNotification();
  const setting = useUserSettings();

  const [selectFilter, setSelectFilter] = useState("");
  const { getFilter, addFilter, removeFilter } = useTableFilter();
  const filters = getFilter(uuid, tableName);
  const formatarg = {
    tabSize: setting.editor.tabSize,
    insertSpaces: true,
  };

  const handleSet = async () => {
    try {
      let data = await extractFn(uuid, editor.getValue());
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
      })}>
      <Box
        sx={{ resize: "both", ...modalStyle, width: "75%", height: "30vh" }}
        ref={setModalBox}>
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
          handleEditor={(e) => {
            setEditor(e);
            e.focus();
            editorPositionToEnd(e);
            e.getModel()[dbpalQuerykey] = new NameAndQueryId(uuid, kind);
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
                      const data = await extractFn(uuid, value);
                      addFilter(uuid, tableName, data);
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
                  removeFilter(uuid, tableName);
                  showNotification({
                    severity: "success",
                    title: "delete filter success",
                  });
                }}>
                <DeleteForeverIcon />
              </IconButton>
              <Autocomplete
                value={selectFilter}
                style={{ flex: 1 }}
                options={filters}
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

function CopySql(props: { sql; style?: CSSProperties }) {
  let { sql, style } = props;
  const { showNotification } = useNotification();
  if (!sql) {
    return <> </>;
  }
  return (
    <Stack style={{ ...style }} direction="row">
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
  dom: Element;
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
  appcodeExtra?: AppCodeExtra;
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

function AiMarkdownModelSwitch(props: { setMarkdownModel; markdownModel: boolean }) {
  const { setMarkdownModel, markdownModel } = props;
  const { getSwitch, setSwitch } = useBoolSwitch();
  const switchKey = "ai-markdown-model";
  const llm = useLLM();
  const vis = useContext(TabActiveContext);
  useEffect(() => {
    if (!llm) {
      return;
    }
    const v = getSwitch(switchKey);
    setMarkdownModel(v);
  }, [getSwitch, llm, setMarkdownModel]);

  useEffect(() => {
    if (!llm) {
      return;
    }
    setSwitch(switchKey, markdownModel);
  }, [llm, markdownModel, setSwitch]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (KeyPressed.editorMarkdown(e) && vis) {
        e.preventDefault();
        setMarkdownModel((e: boolean) => !e);
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

function ShowingCount({ showing, total }) {
  if (showing == total) {
    return <></>;
  }
  return (
    <Typography color="info" style={textEllipsisStyle}>
      showing {showing} of {total}
    </Typography>
  );
}

function FetchTimeAndXXX({ isLoading }) {
  const [fetchTime, setFetchTime] = useState<number | null>(null);
  const [date, setDate] = useState(new Date());
  const fetchBeginTime = useRef(new Date().getTime());
  useEffect(() => {
    if (isLoading) {
      fetchBeginTime.current = new Date().getTime();
      const int = setInterval(() => {
        setFetchTime(new Date().getTime() - fetchBeginTime.current);
      }, 100);
      return () => {
        clearInterval(int);
      };
    } else {
      setFetchTime(new Date().getTime() - fetchBeginTime.current);
      setDate(new Date());
    }
  }, [isLoading]);

  return (
    <>
      <Typography color="info">{date.toLocaleTimeString()}</Typography>
      {!!fetchTime && (
        <Chip
          label={prettyMs(fetchTime)}
          color={fetchTime > 4000 ? "warning" : "info"}
          icon={<IoTimerOutline fontSize="1.2rem" />}
        />
      )}
    </>
  );
}

function FetchTimeAndDataGrid() {
  const apiRef = useGridApiContext();

  // 使用选择器获取当前的 loading 状态
  const isLoading: boolean = useGridSelector(apiRef, gridRowsLoadingSelector);

  return <FetchTimeAndXXX isLoading={isLoading} />;
}

export {
  FetchTimeAndDataGrid,
  FetchTimeAndXXX,
  ShowingCount,
  CreateDatabase,
  CreateSchema,
  SimpleFlexEditor,
  EditorBar,
  MenuConnectionInfo,
  ConnConfigInfo,
  LoadStoreFile,
  AppMarkDown,
  UrlDocument,
  SaveFavoriteSqlView,
  CancelButton,
  SliderSize,
  TableAddButton,
  SelectColumnAsc,
  CreateIndexView,
  TypeSelect,
  ColumnNullness,
  SqlDefaultValue,
  AppDataGridPagination,
  AppDataGridToolBar,
  LastFetchTime,
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
};
