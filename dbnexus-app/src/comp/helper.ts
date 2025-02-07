/// random throw of something
/// used in test


import { languages } from "monaco-editor";
import types from "./ApiStruct";
import { createContext, CSSProperties, ReactNode } from "react";
import { ask } from '@tauri-apps/plugin-dialog';
import { load } from "@tauri-apps/plugin-store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";


interface ConfirmOptions {
  title: string;
  content?: string;
  confirmText?: string;
  cancelText?: string;
}

async function secondConfirm({
  title,
  content,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
}: ConfirmOptions): Promise<boolean> {
  return ask(title, {
    title: content,
    kind: 'error',
    okLabel: confirmText,
    cancelLabel: cancelText,
  });
}

function datagridContainerHeight(length) {
  return length < 20 ? undefined : '78vh';

}


async function randomTestThrowFunction() {
  await delayedReturn(undefined, Math.random() * 900);
  const v = Math.random();
  if (v > 0.8) {
    throw new Error(`this is random error of ${v}`);
  }
}

async function voidTestFunction() {
  try {
    await randomTestThrowFunction();
  } catch (e) {
    throw e;
  }
}

function generateRandomString(length: number = 10): string {
  const characters =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  for (let i = 0; i < length; i++) {
    const randomIndex = Math.floor(Math.random() * characters.length);
    result += characters[randomIndex];
  }
  return result;
}

function generateRandomUint8Array(length: number = 40): Uint8Array {
  let result = [];
  for (let i = 0; i < length; i++) {
    result.push(Math.floor(Math.random() * 255));
  }
  return new Uint8Array(result);
}

function delayedReturn(value: any, delay: number) {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve(value);
    }, delay);
  });
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
function uint8ArrayToUtf8(uint8Array): string {

  const decoder = new TextDecoder('utf-8');
  const str = decoder.decode(uint8Array);

  return str


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
        return languages.CompletionItemKind.Module;
      case 'Schema':
        return languages.CompletionItemKind.Folder;
      case 'Table':
        return languages.CompletionItemKind.File;
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

function extractConfigNameFromUuid(uuid: string): string {
  if (uuid.includes('/')) {
    return uuid.substring(0, uuid.indexOf('/'));
  } else {
    return uuid;
  }
}
function getViewportSize() {
  const width = window.innerWidth; // 获取视口的宽度
  const height = window.innerHeight; // 获取视口的高度

  return { width, height }; // 返回一个对象，包含宽度和高度
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
  if (width - x > 150) {
    // this is click have a lot of space on the right
    ret.left =
      x -
      10 + // 稍微靠左一点
      'px';
  } else {
    ret.right = width - x - 10 + 'px';
  }

  if (height - y > (eheigth)) {
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

enum DataSourceKind {
  Mysql,
  MariaDB,
  TiDB,
  Sqlite,
  Postgres,
  MsSql,
  Redis,
  Mongo,
  Oracle,

}

function connTypeToDataSourceKind(name: string): DataSourceKind {
  switch (name) {
    case 'mysql':
      return DataSourceKind.Mysql;
    case 'mariadb':
      return DataSourceKind.MariaDB;
    case "tidb":
      return DataSourceKind.TiDB;
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
    case 'oracle':
      return DataSourceKind.Oracle;

  }
}
function isTypeLargeText(s: string): boolean {
  const toUpper = s.toUpperCase();
  if (toUpper.includes('TEXT')) {
    return true;
  }
  return false;
}

const appStore = async () => {
  return await load('store.json');
};

enum AppStoreKey {
  UserSetttings = 'user_settings',
  ConnectionConfig = 'connection_config',
  ColumnWithd = 'column_widths'
}

class ColumnWidths {
  widths = {};
  constructor() {

    appStore().then(e => {
      e.get(AppStoreKey.ColumnWithd).then(e => {
        if (e) {
          this.widths = e;
        }
      })
    })
  }

  saveColumnWidths(name: string, width: number) {
    this.widths[name] = width;
    this.save();

  }
  save() {
    appStore().then(e => {
      e.set(AppStoreKey.ColumnWithd, this.widths).catch(e => {
        console.log('save column width failed', e);
      });
    }).catch(e => {
      console.log('save column width failed', e);
    });

  }
  getWidth(key) {
    return this.widths[key]
  }
  reset() {
    this.widths = {};
    this.save();

  }
}
var columnWidths = new ColumnWidths();

const handleCopyToClipboard = (text, showNotification) => {
  writeText(text)
    .then(() => {
      showNotification('success', 'success copy to clipboard.');
    })
    .catch((e) => {
      showNotification('error', 'copy to clipboard failed', e);
    });
};

class TabContent {
  constructor(
    public id: string,
    public header: ReactNode,
    public content: ReactNode
  ) { }
}
const TabsSetContext = createContext<{
  tabs: TabContent[];
  setTabs;
  setSelectIndex;
}>(null);

const ConnTypeContext = createContext<{
  kind: DataSourceKind;
}>(null);

const UuidContext = createContext<{
  uuid: string;
}>(null);

const ContextMenuContext = createContext<{
  setUpMenu: ({ content, style }) => void;
}>(null);

function noUse(...args) {
  //@ts-ignore
  let _args = args;
}
export {
  TabsSetContext,
  ConnTypeContext,
  UuidContext,
  ContextMenuContext,
  TabContent,
  randomTestThrowFunction,
  generateRandomString,
  delayedReturn,
  voidTestFunction,
  generateRandomUint8Array,
  hexTryToUtf8,
  hexToUtf8,
  hexToUint8Array,
  stringToHexArray,
  arrayToHex,
  stringToHex,
  backendCompletionItemToEditor,
  extractConfigNameFromUuid,
  calMenuStyle, chunkArray,
  errToString,
  getViewportSize,
  isTypeLargeText,
  DataSourceKind,
  connTypeToDataSourceKind,
  secondConfirm,
  appStore,
  AppStoreKey,
  columnWidths,
  handleCopyToClipboard,
  datagridContainerHeight,
  uint8ArrayToUtf8,
  noUse
};
