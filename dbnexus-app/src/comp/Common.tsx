import {
  Box,
  Button,
  FormControl,
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Select,
  Stack,
  SxProps,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';

import {
  languages,
  Position as EditorPositon,
  editor,
  Position,
  CancellationToken,
} from 'monaco-editor';
import { types } from './ApiStruct';
import { DataGrid } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import { BSON } from 'bson';
import {
  createContext,
  CSSProperties,
  useContext,
  useEffect,
  useState,
} from 'react';
import { Editor, loader, OnMount } from '@monaco-editor/react';
import { TabContent } from './Tabs';
import RefreshIcon from '@mui/icons-material/Refresh';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import AddIcon from '@mui/icons-material/Add';
import { secondConfirm } from './Confirm';
import { TbDatabasePlus } from 'react-icons/tb';
import LinkIcon from '@mui/icons-material/Link';
import LinkOffIcon from '@mui/icons-material/LinkOff';
import InfoIcon from '@mui/icons-material/Info';
import CloseIcon from '@mui/icons-material/Close';
const modalStyle: SxProps = {
  position: 'absolute',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  bgcolor: 'background.paper',
  border: '1px solid #000',
  boxShadow: 24,
  margin: 0,
  p: 4,
};
import { BsDatabaseAdd } from 'react-icons/bs';
import { load } from '@tauri-apps/plugin-store';
import {
  mssqlKeywords,
  mysqlKeywords,
  postgresKeywords,
  redisKeywords,
  sqliteKeywords,
} from './keywords';

const appStore = async () => {
  return await load('store.json');
};

const getNamedConfig = async (
  name: string
): Promise<types.DBConnectionConfig> => {
  let x = await (
    await appStore()
  ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
  if (!x) {
    return undefined;
  }
  return x.find((x) => x.name == name);
};

enum AppStoreKey {
  UserSetttings = 'user_settings',
  ConnectionConfig = 'connection_config',
}

function MetaValuesView(props: { values: types.MetaValue[]; flex?: number }) {
  let { values, flex } = props;
  if (flex == undefined) {
    flex = 1;
  }

  let columns = [
    { field: 'name', flex },
    { field: 'value', flex },
    { field: 'desc', flex },
  ];

  values.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidv4();
    }
  });

  return (
    <DataGrid
      rowHeight={datagridRowHeight}
      columnHeaderHeight={datagridHeaderHeight}
      columns={columns}
      rows={values}
    ></DataGrid>
  );
}
function SelectPageSize({
  width = '130px',
  variant = 'outlined',
  tital = 'select page size',
  pageSize,
  handleChange,
}) {
  return (
    <Box width={width}>
      <FormControl
        fullWidth
        //@ts-ignore
        variant={variant}
        size="small"
      >
        <InputLabel id="simple-select-label">{tital}</InputLabel>
        <Select
          labelId="simple-select-label"
          id="simple-select"
          value={pageSize}
          onChange={(e) => {
            handleChange(e.target.value);
          }}
          label="select page size"
        >
          {pageSizeOptions.map((e) => (
            <MenuItem key={e} value={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
    </Box>
  );
}

// redis的数据存储的时候都是二进制 有时候包含的是有效的
// utf8字符前台的处理的最好当成utf8来处理
// 不然当成二进制数据来处理呢
// 后台放回数据的时候都是 hex string模式

function hexTryToUtf8(hex): string | Uint8Array {
  if (!hex) {
    return '';
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  // 将字节数组转换为 UTF-8 字符串
  let utf8String: string;

  try {
    utf8String = new TextDecoder('utf-8', { fatal: true }).decode(
      new Uint8Array(bytes)
    );
  } catch {
    return new Uint8Array(bytes);
  }
  return utf8String;
}

function hexToUint8Array(hex) {
  if (!hex) {
    return new Uint8Array([]);
  }
  const bytes = [];

  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  return new Uint8Array(bytes);
}
function hexToUtf8(hex: string): string {
  if (!hex) {
    return '';
  }
  // 将十六进制字符串转换为字节数组
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
  return new TextDecoder('utf-8').decode(new Uint8Array(bytes));
}

function stringToHexArray(str: string): Uint8Array {
  // 创建一个 Uint8Array 来存储字节
  const bytes = new TextEncoder().encode(str);
  return bytes;
}

function arrayToHex(uint8Array: Uint8Array | number[]) {
  return Array.from(uint8Array) // 将 Uint8Array 转换为普通数组
    .map((byte) => byte.toString(16).padStart(2, '0')) // 转换每个字节为十六进制并补齐
    .join(''); // 连接为一个字符串
}

function stringToHex(str): string {
  return arrayToHex(stringToHexArray(str));
}

function backendCompletionItemToEditor(
  xs: types.CompletionItem[]
): languages.CompletionList {
  function getKind(kind: types.CompletionKind): languages.CompletionItemKind {
    switch (kind) {
      case 'Command':
        return languages.CompletionItemKind.Keyword;
      case 'KeyWord':
        return languages.CompletionItemKind.Keyword;
      case 'Function':
        return languages.CompletionItemKind.Function;
      case 'Database':
        return languages.CompletionItemKind.Folder;
      case 'Schema':
        return languages.CompletionItemKind.Folder;
      case 'Table':
        return languages.CompletionItemKind.Module;
      case 'Column':
        return languages.CompletionItemKind.Field;
      case 'Text':
        return languages.CompletionItemKind.Text;
      case 'Type':
        return languages.CompletionItemKind.Class;
      case 'Index':
        return languages.CompletionItemKind.Reference;
      case 'Sequence':
        return languages.CompletionItemKind.Reference;
      case 'Stage':
        return languages.CompletionItemKind.Reference;
      case 'Trigger':
        return languages.CompletionItemKind.Function;
      case 'Event':
        return languages.CompletionItemKind.Event;
      case 'Procedure':
        return languages.CompletionItemKind.Function;
      case 'Extension':
        return languages.CompletionItemKind.Module;
      case 'User':
        return languages.CompletionItemKind.User;
      case 'Role':
        return languages.CompletionItemKind.User;
      case 'Constraint':
        return languages.CompletionItemKind.Operator;
      case 'Partition':
        return languages.CompletionItemKind.Constant;
      case 'PrepareStatement':
        return languages.CompletionItemKind.Function;
      case 'Pragma':
        return languages.CompletionItemKind.Keyword;
      case 'Policy':
        return languages.CompletionItemKind.Keyword;
      case 'Variable':
        return languages.CompletionItemKind.Variable;
      case 'Rule':
        return languages.CompletionItemKind.Operator;
    }
  }
  return {
    suggestions: xs.map((e): languages.CompletionItem => {
      return {
        label: e.item,
        kind: getKind(e.kind),
        insertText: e.item,
        range: undefined,
      };
    }),
  };
}

class DocsAndCount {
  constructor(public docs: BSON.Document[], public total: number) {}
}
enum MongoDataViewModel {
  List,
  Object,
}

const pageSizeOptions = [10, 20, 50, 100];

// TODO Add more
// if type large text should use multiline
// textfield

function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  if (toUpper.includes('TEXT')) {
    return true;
  }
  return false;
}

function EditorBar(props: {
  left?: any;
  pos: EditorPositon;
  style: CSSProperties | undefined;
}) {
  let { left, pos, style } = props;
  return (
    <Stack justifyContent="space-between" direction="row" style={style}>
      <div style={{ width: '40%' }}> {left}</div>
      <Stack width="40%" direction="row" spacing={1}>
        <span>Ln {pos ? pos.lineNumber : ''}</span>
        <span>Column {pos ? pos.column : ''}</span>
      </Stack>
    </Stack>
  );
}

function MenuCreateDatabase({ handleCreate }) {
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}
    >
      <ListItemIcon>
        <BsDatabaseAdd></BsDatabaseAdd>
      </ListItemIcon>
      <ListItemText> create database </ListItemText>
    </MenuItem>
  );
}

function MenuCreateSchema(props: { handleCreate; kind: DataSourceKind }) {
  const { handleCreate, kind } = props;
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}
    >
      <ListItemIcon>
        <TbDatabasePlus></TbDatabasePlus>
      </ListItemIcon>
      <ListItemText>
        create
        {kind == DataSourceKind.MsSql || kind == DataSourceKind.Postgres
          ? ' schema'
          : ' database'}
      </ListItemText>
    </MenuItem>
  );
}

function CreateDatabase({ uuid, handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const [database, setDatabase] = useState('');

  const handleCreate = () => {
    if (!database) {
      return;
    }

    invoke('create_database', { name: uuid, database })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  return (
    <Stack>
      <Stack>
        <TextField
          value={database}
          onChange={(e) => {
            setDatabase(e.target.value);
          }}
          error={database == ''}
          label="dadtabase name"
        ></TextField>
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
  kind: DataSourceKind;
  schemas: string[];
}) {
  //@ts-ignore
  const { uuid, handleClose, kind, schemas } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useContext(NotificationContext);
  const [schema, setSchema] = useState('');

  const handleCreate = () => {
    if (!schema) {
      showNotification('error', 'Schema is empty');
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification('error', `'${schema}' already exists.`);
      return;
    }
    invoke('create_schema', { name: uuid, schema })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  if (kind == DataSourceKind.Redis) {
    // redis have special create
    const range = [];
    for (let i = 0; i < 16; i++) {
      range.push('db' + i);
    }

    return (
      <Stack>
        <Stack direction="row" style={{ flexWrap: 'wrap' }}>
          {range
            .filter((e) => !schemaSet.has(e))
            .map((e) => (
              <Button
                key={e}
                onClick={() => {
                  setSchema(e);
                }}
              >
                {e}
              </Button>
            ))}
        </Stack>
        <Stack>
          <TextField
            value={schema}
            onChange={(e) => {
              setSchema(e.target.value);
            }}
            label="name"
          ></TextField>
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

  return (
    <Stack>
      <Stack>
        <TextField
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          label="name"
        ></TextField>
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

async function dropDatabase(props: {
  uuid: string;
  database;
  showNotification;
  okCallback?;
}) {
  const { uuid, database, showNotification, okCallback } = props;

  const confirm = await secondConfirm({
    title: `drop database ${database}`,
    confirmText: 'drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_database', { name: uuid, database })
    .then(() => {
      showNotification('success', 'Drop OK');
      if (okCallback) {
        okCallback();
      }
    })
    .catch((e) => {
      showNotification('error', 'Drop failure', e);
    });
}

async function dropSchema({ uuid, schema, showNotification, okCallback }) {
  const confirm = await secondConfirm({
    title: `drop ${schema}`,
    confirmText: 'drop',
  });
  if (!confirm) {
    return;
  }
  invoke('drop_schema', { name: uuid, schema })
    .then(() => {
      showNotification('success', 'Drop OK');
      okCallback();
    })
    .catch((e) => {
      showNotification('error', 'Drop failure', e);
    });
}

function countLines(str) {
  if (!str) return undefined; // 如果字符串为空，返回 0
  return str.split('\n').length; // 按换行符分割并计算长度
}

function editorInitHeight(str, maxPrecent = 0.5): number {
  let lines = countLines(str);
  if (!lines) {
    lines = 10;
  }
  let height = lines * 22;
  let viewHeigth = getViewportSize().height;
  let maxHeigth = viewHeigth * maxPrecent;
  if (height > maxHeigth) {
    height = maxHeigth;
  }
  return height;
}

function SimpleResizeEditor(props: {
  language: string;
  value: string;
  onMount?: OnMount;
  options?: editor.IStandaloneEditorConstructionOptions;
  style?: CSSProperties;
  minHeigth?: number;
}) {
  let { language, value, onMount, options, style, minHeigth } = props;

  let height = editorInitHeight(value);
  if (minHeigth && height < minHeigth) {
    height = minHeigth;
  }
  const { theme } = useContext(SettingsContext);
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
        marginRight: '20px',
        ...style,
      }}
    >
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          minHeight: 0,
          width: '99%',
          height: height + 'px',
        }}
      >
        <Editor
          options={options}
          onMount={onMount}
          language={language}
          value={value}
          theme={theme == 'dark' ? 'vs-dark' : 'light'}
        ></Editor>
      </div>
    </div>
  );
}

const datagridRowHeight = 25;
const datagridHeaderHeight = 36;

const TabsSetContext = createContext<{
  tabs: TabContent[];
  setTabs;
  setSelectIndex;
}>(null);

enum DataSourceKind {
  Mysql,
  MariaDB,
  Sqlite,
  Postgres,
  MsSql,
  Redis,
  Mongo,
}

function connTypeToDataSourceKind(name: string): DataSourceKind {
  switch (name) {
    case 'mysql':
      return DataSourceKind.Mysql;
    case 'mariadb':
      return DataSourceKind.MariaDB;
    case 'redis':
      return DataSourceKind.Redis;
    case 'mongo':
      return DataSourceKind.Mongo;
    case 'sqlserver':
      return DataSourceKind.MsSql;
    case 'sqlite':
      return DataSourceKind.Sqlite;
    case 'postgres':
      return DataSourceKind.Postgres;
  }
}
const ConnTypeContext = createContext<{
  kind: DataSourceKind;
}>(null);

const UuidContext = createContext<{
  uuid: string;
}>(null);

const MenuContext = createContext<{
  setUpMenu: ({ content, style }) => void;
}>(null);

const msgTimeout = 5000;

function tabsHasId(tabs: TabContent[], id: string): number | null {
  for (let i = 0; i < tabs.length; i++) {
    let tab = tabs[i];
    if (tab.id == id) {
      return i;
    }
  }
  return null;
}

function extraParametersFromMetaValues(
  values: types.MetaValue[]
): string | null {
  for (let i = 0; i < values.length; i++) {
    let v = values[i];
    if (v.name == 'parameters') {
      return v.value;
    }
  }
  return null;
}

interface DefAndMetas {
  def: string;
  metas: types.MetaValue[];
}
const progressBarSize = '30px';
const progressBarSizeSmall = '20px';

function getViewportSize() {
  const width = window.innerWidth; // 获取视口的宽度
  const height = window.innerHeight; // 获取视口的高度
  return { width, height }; // 返回一个对象，包含宽度和高度
}

function MenuRefresh({ reload }) {
  return (
    <MenuItem
      onClick={() => {
        reload();
      }}
    >
      <ListItemIcon>
        <RefreshIcon></RefreshIcon>
      </ListItemIcon>
      <ListItemText> refresh </ListItemText>
    </MenuItem>
  );
}

function MenuConnectionInfo({ uuid }) {
  const { tabs, setSelectIndex, setTabs } = useContext(TabsSetContext);

  const handleInfo = async () => {
    let id = uuid + '-info';
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [
          ...e,
          new TabContent(id, uuid, <ConnInfo uuid={uuid}></ConnInfo>),
        ];
      });
    }
  };
  return (
    <MenuItem onClick={handleInfo}>
      <ListItemIcon>
        <InfoIcon></InfoIcon>
      </ListItemIcon>
      <ListItemText> info </ListItemText>
    </MenuItem>
  );
}

function MenuDeleteConfig({ name }) {
  const { showNotification } = useContext(NotificationContext);
  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: `drop connection '${name}'`,
      confirmText: 'drop',
    });
    if (!confirm) {
      return;
    }

    try {
      let old = await (
        await appStore()
      ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
      let newdata = old.filter((e) => e.name != name);
      await (await appStore()).set(AppStoreKey.ConnectionConfig, newdata);
      await invoke('close_conn', { name });
    } catch (e) {
      showNotification('error', 'drop connection failed', e);
    }
  };

  return (
    <MenuItem
      onClick={() => {
        handleDelete();
      }}
    >
      <ListItemIcon>
        <CloseIcon></CloseIcon>
      </ListItemIcon>
      <ListItemText> drop connection </ListItemText>
    </MenuItem>
  );
}

function MenuNewQuery({ handle }) {
  return (
    <MenuItem
      onClick={() => {
        handle();
      }}
    >
      <ListItemIcon>
        <AddIcon></AddIcon>
      </ListItemIcon>
      <ListItemText> New Query </ListItemText>
    </MenuItem>
  );
}

function MenuOpenConn({ handleOpend, name }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={() => {
        invoke('open_conn', { name })
          .then(() => {
            handleOpend();
          })
          .catch((e) => {
            showNotification('error', 'open connection failed', e);
          });
      }}
    >
      <ListItemIcon>
        <LinkIcon></LinkIcon>
      </ListItemIcon>
      <ListItemText> open connection </ListItemText>
    </MenuItem>
  );
}

function MenuCloseConn({ handleClose, name }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={() => {
        invoke('close_conn', { name })
          .then(() => {
            handleClose();
          })
          .catch((e) => {
            showNotification('error', 'open connection failed', e);
          });
      }}
    >
      <ListItemIcon>
        <LinkOffIcon></LinkOffIcon>
      </ListItemIcon>
      <ListItemText> close connection </ListItemText>
    </MenuItem>
  );
}

function calMenuStyle(e, eheigth: number): CSSProperties {
  e.preventDefault();
  const x = e.clientX;
  const y = e.clientY;
  let ret: CSSProperties = {};
  const leftSizeMenuDom = document.getElementById('menu-div');
  leftSizeMenuDom.style.display = 'block';
  const { height, width } = getViewportSize();
  ret.top = 'auto';
  ret.bottom = 'auto';
  ret.left = 'auto';
  ret.right = 'auto';
  const menuWidth = 150;
  if (width - x > menuWidth) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      'px';
  } else {
    ret.right = width - x - 10 + 'px';
  }

  if (height - y > eheigth) {
    // 这个是点击的位置下面有很大的空间
    ret.top = y - 10 + 'px';
  } else {
    ret.bottom = height - y - 10 + 'px';
  }
  return ret;
}

function chunkArray(array: any[], chunkSize: number): any[][] {
  const result = [];
  for (let i = 0; i < array.length; i += chunkSize) {
    result.push(array.slice(i, i + chunkSize));
  }
  return result;
}

function errToString(message) {
  if (typeof message == 'object' && 'reason' in message) {
    // TODO show message
    message = message['reason'];
  }
  if (typeof message == 'object' && message instanceof Error) {
    // TODO show message
    message = message.toString();
  }
  return message;
}

function ConnStatus({ uuid }) {
  const [status, setStatus] = useState('');
  const [err, setErr] = useState('');
  const reloadStatus = () => {
    invoke<string>('check_conn', { name: uuid })
      .then((ping) => {
        setStatus(ping);
      })
      .catch((e) => {
        setErr(errToString(e));
      });
  };

  useEffect(() => {
    reloadStatus();
    let r = setInterval(() => {
      reloadStatus();
    }, 5000);
    return () => {
      clearInterval(r);
    };
  }, []);

  return (
    (status || err) &&
    (status ? (
      <Tooltip title="ping time">
        <Typography color="green"> {status} </Typography>
      </Tooltip>
    ) : (
      <Tooltip title={err}>
        <Typography color="error"> err </Typography>
      </Tooltip>
    ))
  );
}
class UserSettings {
  constructor(public theme: string = 'light', public pageSize: number = 100) {}
}

const SettingsContext = createContext<UserSettings>(new UserSettings());

const OpenedContext = createContext<{ opened: boolean; setOpened }>(null);

function ConnInfo(props: { uuid }) {
  let { uuid } = props;
  const { showNotification } = useContext(NotificationContext);
  const [metas, setMetas] = useState<types.MetaValue[]>([]);
  const handleInfo = async () => {
    try {
      let metas = await invoke<types.MetaValue[]>('server_metas', {
        name: uuid,
      });
      let config = await getNamedConfig(uuid);
      let configMetas = [];
      switch (config.conn_type) {
        case 'mysql':
        case 'mariadb':
        case 'postgres':
        case 'redis':
        case 'mongo':
          configMetas = [
            { name: 'host', value: config.host } as types.MetaValue,
            { name: 'port', value: config.port + '' } as types.MetaValue,
            { name: 'user', value: config.username } as types.MetaValue,
          ];
          break;
        case 'sqlite':
          configMetas = [
            { name: 'path', value: config.path } as types.MetaValue,
          ];
          break;
      }
      setMetas([...configMetas, ...metas]);
    } catch (e) {
      showNotification('error', 'call server_metas failed', e);
      console.log(e);
    }
  };

  useEffect(() => {
    handleInfo();
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Button variant="contained" color="primary" onClick={handleInfo}>
          reload
        </Button>
      </Stack>
      <div style={{ display: 'grid' }}>
        <MetaValuesView values={metas}></MetaValuesView>
      </div>
    </Stack>
  );
}
function reloadOK(showNotification) {
  showNotification('success', 'Reload OK');
}
const registeredLanguage = new Set<string>();
const VisiableContext = createContext(true);

function initLanguageXXX(
  language: string,
  keyword,
  autoCompletion: (
    text: string,
    pos: Position
  ) => Promise<languages.CompletionList>
) {
  loader.init().then((monaco) => {
    monaco.languages.register({ id: language });
    monaco.languages.setMonarchTokensProvider(language, {
      ignoreCase: true,
      tokenizer: {
        root: [[keyword, 'keyword']],
      },
    });

    monaco.languages.registerCompletionItemProvider(language, {
      provideCompletionItems: (
        model: editor.ITextModel,
        position: Position,
        _context: any,
        _token: CancellationToken
      ) => {
        let text = model.getValue();
        return autoCompletion(text, position);
      },
    });
  });
}

// uuid as language identifier.
function initMonacoLanague(uuid: string, kind: DataSourceKind) {
  if (registeredLanguage.has(uuid)) {
    return;
  }
  const auto_completion = async (
    sql: string,
    pos: Position
  ): Promise<languages.CompletionList> => {
    try {
      let completionItem = await invoke<types.CompletionItem[]>(
        'auto_completion',
        {
          name: uuid,
          sql,
          line: pos.lineNumber,
          col: pos.column,
        }
      );

      let suggestions = backendCompletionItemToEditor(completionItem);
      return suggestions;
    } catch (e) {
      console.log('completion server error:', e);
      throw e;
    }
  };

  switch (kind) {
    case DataSourceKind.Mysql:
    case DataSourceKind.MariaDB:
      initLanguageXXX(uuid, mysqlKeywords, auto_completion);
      break;
    case DataSourceKind.Sqlite:
      initLanguageXXX(uuid, sqliteKeywords, auto_completion);
      break;
    case DataSourceKind.Postgres:
      initLanguageXXX(uuid, postgresKeywords, auto_completion);
      break;
    case DataSourceKind.MsSql:
      initLanguageXXX(uuid, mssqlKeywords, auto_completion);
      break;
    case DataSourceKind.Redis:
      initLanguageXXX(uuid, redisKeywords, auto_completion);
      break;
    default:
      throw `impl this ${kind}`;
  }
  registeredLanguage.add(uuid);
}

function extractConfigNameFromUuid(uuid: string): string {
  if (uuid.includes('/')) {
    return uuid.substring(0, uuid.indexOf('/'));
  } else {
    return uuid;
  }
}

async function makeQueryConnInfo(uuid: string) {
  const name = extractConfigNameFromUuid(uuid);
  const config = await getNamedConfig(name);
  if (!config) {
    return;
  }
  let content;
  switch (config.conn_type) {
    case 'mysql':
    case 'mariadb':
    case 'postgres':
    case 'sqlserver':
      content = (
        <>
          <strong>{config.conn_type}</strong>
          <span>
            {config.username}@{config.host}:{config.port}/
          </span>
          <span>{uuid} </span>
        </>
      );
      break;
    case 'sqlite':
      content = (
        <>
          <strong>sqlite</strong>
          <span> {config.path}</span>
        </>
      );
      break;
    case 'redis':
      content = (
        <>
          <strong>{config.conn_type}</strong>
          <span>
            {config.username} {config.username && '@'}
            {config.host}:{config.port}/{uuid}
          </span>
        </>
      );
      break;
  }
  return (
    <Stack direction="row" spacing={1}>
      {content}
    </Stack>
  );
}

export {
  modalStyle,
  SelectPageSize,
  hexTryToUtf8,
  stringToHexArray,
  arrayToHex,
  hexToUtf8,
  hexToUint8Array,
  stringToHex,
  MetaValuesView,
  backendCompletionItemToEditor,
  DocsAndCount,
  MongoDataViewModel,
  pageSizeOptions,
  isTypeLargeText,
  EditorBar,
  datagridRowHeight,
  datagridHeaderHeight,
  countLines,
  SimpleResizeEditor,
  editorInitHeight,
  TabsSetContext,
  tabsHasId,
  msgTimeout,
  UuidContext,
  DataSourceKind,
  connTypeToDataSourceKind,
  progressBarSize,
  MenuContext,
  calMenuStyle,
  MenuRefresh,
  ConnTypeContext,
  chunkArray,
  progressBarSizeSmall,
  extraParametersFromMetaValues,
  MenuDeleteConfig,
  MenuNewQuery,
  ConnStatus,
  SettingsContext,
  AppStoreKey,
  appStore,
  UserSettings,
  VisiableContext,
  MenuOpenConn,
  OpenedContext,
  MenuCloseConn,
  MenuConnectionInfo,
  getNamedConfig,
  MenuCreateDatabase,
  CreateDatabase,
  CreateSchema,
  dropDatabase,
  dropSchema,
  MenuCreateSchema,
  reloadOK,
  registeredLanguage,
  initMonacoLanague,
  makeQueryConnInfo,
};

export type { DefAndMetas };
