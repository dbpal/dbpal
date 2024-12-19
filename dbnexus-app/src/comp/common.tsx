import {
  Box,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
} from '@mui/material';

import { languages, Position as EditorPositon, editor } from 'monaco-editor';
import { types } from './ApiStruct';
import { DataGrid } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import { BSON } from 'bson';
import { createContext, CSSProperties } from 'react';
import { Editor, OnMount } from '@monaco-editor/react';
import { TabContent } from './Tabs';

// TOOD 适配黑暗模式
const modalStyle = {
  position: 'absolute',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  bgcolor: 'background.paper',
  border: '1px solid #000',
  boxShadow: 24,
  padding: 0,
  margin: 0,
  p: 4,
};

function MetaValues(props: { values: types.MetaValue[] }) {
  let { values } = props;
  let columns = [
    { field: 'name' },
    { field: 'value' },
    { field: 'desc', flex: 1 },
  ];
  values.forEach((e) => {
    //@ts-ignore
    e.id = uuidv4();
  });

  return (
    <>
      <DataGrid
        rowHeight={datagridRowHeight}
        columnHeaderHeight={datagridHeaderHeight}
        columns={columns}
        rows={values}
      ></DataGrid>
    </>
  );
}
function SelectPageSize({
  width = '130px',
  variant = 'outlined',
  tital = 'select page size',
  pageSize = 50,
  onChange = (_: number) => {
    // a function which will do nothing.
  },
}) {
  return (
    <Box width={width}>
      <FormControl
        fullWidth
        // @ts-ignore
        variant={variant}
        size='small'
      >
        <InputLabel id='simple-select-label'>{tital}</InputLabel>
        <Select
          labelId='simple-select-label'
          id='simple-select'
          value={pageSize}
          onChange={(e) => {
            onChange(e.target.value as number);
          }}
          label='select page size'
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
    <Stack justifyContent='space-between' direction='row' style={style}>
      <div style={{ width: '40%' }}> {left}</div>
      <Stack width='40%' direction='row' spacing={1}>
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
}) {
  let { language, value, onMount, options } = props;
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        resize: 'vertical',
        overflow: 'auto',
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
const defaultPageSize = 10;

const TabsSetContext = createContext<{
  tabs: TabContent[];
  setTabs;
  setSelectIndex;
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

interface DefAndMetas {
  def: string;
  metas: types.MetaValue[];
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
  MetaValues,
  backendCompletionItemToEditor,
  DocsAndCount,
  MongoDataViewModel,
  pageSizeOptions,
  isTypeLargeText,
  EditorBar,
  datagridRowHeight,
  datagridHeaderHeight,
  defaultPageSize,
  countLines,
  SimpleResizeAbleEditor,
  editorInitHeight,
  TabsSetContext,
  tabsHasId,
  msgTimeout,
};

export type { DefAndMetas };
