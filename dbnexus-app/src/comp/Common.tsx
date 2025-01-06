import {
  Box,
  FormControl,
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Select,
  Stack,
  SxProps,
  Tooltip,
  Typography,
} from '@mui/material';

import { languages, Position as EditorPositon, editor } from 'monaco-editor';
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
import { Editor, OnMount } from '@monaco-editor/react';
import { TabContent } from './Tabs';
import RefreshIcon from '@mui/icons-material/Refresh';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import AddIcon from '@mui/icons-material/Add';
import { secondConfirm } from './Confirm';
import CloseIcon from '@mui/icons-material/Close';
import LinkIcon from '@mui/icons-material/Link';
import LinkOffIcon from '@mui/icons-material/LinkOff';
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

import { load } from '@tauri-apps/plugin-store';
const appStore = async () => {
  return await load('store.json');
};

enum AppStoreKey {
  UserSetttings = 'user_settings',
  ConnectionConfig = 'connection_config',
}

function MetaValues(props: { values: types.MetaValue[] }) {
  let { values } = props;
  let columns = [
    { field: 'name', flex: 1 },
    { field: 'value', flex: 1 },
    { field: 'desc', flex: 1 },
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
        // @ts-ignore
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
    }
  }
  return {
    suggestions: xs.map((e): languages.CompletionItem => {
      return {
        label: e.name,
        kind: getKind(e.kind),
        insertText: e.name,
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

function countLines(str) {
  if (!str) return 0; // 如果字符串为空，返回 0
  return str.split('\n').length; // 按换行符分割并计算长度
}

function editorInitHeight(str): number {
  let lines = countLines(str);
  if (!lines) {
    lines = 10;
  }
  return lines * 22;
}

function SimpleResizeAbleEditor(props: {
  language: string;
  value: string;
  onMount?: OnMount;
  options?: editor.IStandaloneEditorConstructionOptions;
  style?: CSSProperties;
}) {
  let { language, value, onMount, options, style } = props;
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
        ...style,
      }}
    >
      <div
        style={{
          flexGrow: 1,
          flexShrink: 1,
          minHeight: 0,
          height: editorInitHeight(value) + 'px',
        }}
      >
        <Editor
          options={options}
          onMount={onMount}
          language={language}
          value={value}
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
        <DeleteForeverIcon color="error"></DeleteForeverIcon>
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
  const leftSizeMenuDom = document.getElementById('left-side-menu');
  leftSizeMenuDom.style.display = 'block';
  const { height, width } = getViewportSize();
  ret.top = 'auto';
  ret.bottom = 'auto';
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

const VisiableContext = createContext(true);
export {
  modalStyle,
  SelectPageSize,
  hexTryToUtf8,
  stringToHexArray,
  arrayToHex,
  hexToUtf8,
  hexToUint8Array,
  stringToHex,
  MetaValues,
  backendCompletionItemToEditor,
  DocsAndCount,
  MongoDataViewModel,
  pageSizeOptions,
  isTypeLargeText,
  EditorBar,
  datagridRowHeight,
  datagridHeaderHeight,
  countLines,
  SimpleResizeAbleEditor,
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
};

export type { DefAndMetas };
