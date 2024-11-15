import {
  Alert,
  Box,
  Button,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Select,
  Stack,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import { useState } from 'react';

const errOkHoldTime = 5000;

// TOOD 适配黑暗模式
const modalStyle = {
  position: 'absolute',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  bgcolor: 'background.paper',
  border: '1px solid #000',
  boxShadow: 24,
  p: 4,
};

function SelectPageSize({
  width = '130px',
  variant = 'outlined',
  tital = 'select page size',
  pageSize = 50,
  setPageSize = (_: number) => {
    // a function which will do nothing.
  },
}) {
  return (
    <Box width={width}>
      <FormControl fullWidth variant={variant}>
        <InputLabel id='simple-select-label'>{tital}</InputLabel>
        <Select
          labelId='simple-select-label'
          id='simple-select'
          value={pageSize}
          onChange={(e) => {
            setPageSize(e.target.value as number);
          }}
          label='select page size'
        >
          <MenuItem value={50}>50</MenuItem>
          <MenuItem value={100}>100</MenuItem>
          <MenuItem value={200}>200</MenuItem>
          <MenuItem value={500}>500</MenuItem>
          <MenuItem value={1000}>1000</MenuItem>
          <MenuItem value={2000}>2000</MenuItem>
          <MenuItem value={5000}>5000</MenuItem>
          <MenuItem value={10000}>10000</MenuItem>
        </Select>
      </FormControl>
    </Box>
  );
}

function youNeedImplThis(name: string) {
  return function (): void {
    throw `You need impl '${name}'`;
  };
}

function CloseButton({ handleClose = youNeedImplThis('handleClose') }) {
  return (
    <IconButton onClick={handleClose} edge='end' aria-label='delete'>
      <CloseIcon />
    </IconButton>
  );
}

function CloseSpace({ handleClose = youNeedImplThis('handleClose') }) {
  return (
    <Stack direction='row' alignItems='right' justifyContent='space-between'>
      <span></span>
      <CloseButton handleClose={handleClose} />
    </Stack>
  );
}

// when u doing some dangerous operation
// u need second confirm about this.

function SecondConfirm(props: {
  msg: string;
  buttonName: string;
  operationOP: () => Promise<boolean>; // boolean used to disable button
  handleClose: () => void;
}) {
  const { msg, operationOP, handleClose, buttonName } = props;
  const [disableButton, setDisableButton] = useState(false);
  return (
    <Stack>
      <CloseSpace handleClose={handleClose}></CloseSpace>
      <Alert severity='error' color='error'>
        {msg}
      </Alert>
      <Button
        disabled={disableButton}
        variant='contained'
        color='error' // 使用 MUI 的 error 颜色
        onClick={() => {
          operationOP().then((disable) => {
            setDisableButton(disable);
          });
        }}
      >
        {buttonName}
      </Button>
    </Stack>
  );
}

// redis的数据存储的时候都是二进制 有时候包含的是有效的
// utf8字符前台的处理的最好当成utf8来处理
// 不然当成二进制数据来处理呢
// 后台放回数据的时候都是 hex string模式

function hexTryToUtf8(hex): string | number[] {
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
    return bytes;
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

export {
  errOkHoldTime,
  modalStyle,
  SelectPageSize,
  CloseButton,
  CloseSpace,
  hexTryToUtf8,
  stringToHexArray,
  arrayToHex,
  hexToUtf8,
  hexToUint8Array,
  stringToHex,
  SecondConfirm,
};
