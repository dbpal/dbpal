import React, { createRef, useEffect, useRef } from 'react';
import ReactDOM from 'react-dom/client';

import apis from './ApiStruct.ts';
import {
  Alert,
  Box,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tabs,
  Tooltip,
} from '@mui/material';
import Paper from '@mui/material/Paper';

import { testExampleQueryResp } from './exampleTest.ts';
import { generateRandomString } from './helper.ts';

const DEFAULT_MIN_WIDTH_CELL = 70;
const DEFAULT_MAX_WIDTH_CELL = 800;

function QueryResp(props: {
  data: apis.MuiDataTableListResp;
  order: string;
  orderBy: string;
  reloadSort: (order: string, orderBy: string) => void;
}) {
  let { data } = props;

  const columnRefs = data.fields.map(() => createRef());
  let columnsWidth = data.fields.map(() => {
    return { minWidth: 100, maxWidth: 600 };
  });

  const isResizing = useRef(-1);

  const setCursorDocument = (isResizing: boolean) => {
    document.body.style.cursor = isResizing ? 'col-resize' : 'auto';
  };

  // function getTitalTooltipTitle(f: apis.Field): string {
  //   let x = f.typ;
  //   if (f.is_auto_increment) {
  //     x += ` | auto increment`;
  //   }
  //   if (f.default_value) {
  //     x += ` | defaultValue(${f.default_value})`;
  //   }
  //   return x;
  // }

  const handleOnMouseUp = () => {
    console.log('end resize');
    isResizing.current = -1;
    setCursorDocument(false);
  };

  // const handleSort = (name: string) => {
  //   let newOrder = order == 'asc' ? 'desc' : 'asc';
  //   reloadSort(name, newOrder);
  // };

  const onClickResizeColumn = (index: number) => {
    console.log('start resize');
    isResizing.current = index;
    setCursorDocument(true);
  };
  const adjustWidthColumn = (index, width) => {
    const minWidth = columnsWidth[index]?.minWidth ?? DEFAULT_MIN_WIDTH_CELL;
    const maxWidth = columnsWidth[index]?.maxWidth ?? DEFAULT_MAX_WIDTH_CELL;
    const newWidth =
      width > maxWidth ? maxWidth : width < minWidth ? minWidth : width;

    columnRefs[index].current.parentElement.style.width = newWidth + 'px';
  };

  const handleOnMouseMove = (e) => {
    if (isResizing.current >= 0) {
      const newWidth =
        e.clientX -
        columnRefs[
          isResizing.current
        ].current.parentElement?.getBoundingClientRect().left;
      adjustWidthColumn(isResizing.current, newWidth);
    }
  };

  useEffect(() => {
    document.onmousemove = handleOnMouseMove;
    document.onmouseup = handleOnMouseUp;
    return () => {
      document.onmousemove = null;
      document.onmouseup = null;
    };
  }, []);

  return (
    <TableContainer component={Paper}>
      <Table className={'table'} sx={{ minWidth: 650 }}>
        <TableHead>
          <TableRow>
            {data.fields.map((field, index) => {
              return (
                <TableCell
                  key={index}
                  sx={{ border: '1px solid black' }}
                  className={'tableCell resizable'}
                  align='center'
                >
                  {field.name}
                  <div
                    onMouseDown={() => onClickResizeColumn(index)}
                    ref={columnRefs[index]}
                    className={'resizeLine'}
                  ></div>
                </TableCell>
              );
            })}
            <TableCell className='tableCell emtpyCell' />
          </TableRow>
        </TableHead>
        <TableBody>
          {/* <TableCell className='tableCell emtpyCell' /> */}
        </TableBody>
      </Table>
    </TableContainer>
  );
}

/*
  multi resp have this rust structure
[
  {
    "Err": "this a error"
  },
  {
    "Ok": {
      "fields": [],
      "data": []
    }
  }
]
*/

function testExampleMultiQueryResp(length: number = 3) {
  let ret = [];
  for (let i = 0; i < length; i++) {
    if (Math.random() > 0.66) {
      ret.push({ Err: `This is random error.` });
    } else {
      ret.push({ Ok: testExampleQueryResp() });
    }
  }

  return ret;
}

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
  idPrefix: string;
}

function CustomTabPanel(props: TabPanelProps) {
  const { children, value, index, idPrefix, ...other } = props;

  return (
    <div
      role='tabpanel'
      hidden={value !== index}
      id={`${idPrefix}-tabpanel-${index}`}
      aria-labelledby={`${idPrefix}-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

function a11yProps(idPrefix: string, index: number) {
  return {
    id: `${idPrefix}-tab-${index}`,
    'aria-controls': `${idPrefix}-tabpanel-${index}`,
  };
}

function MultiQueryResp(props: { data: any[] }) {
  let { data } = props;
  const [value, setValue] = React.useState(0);
  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };
  let idPrefix = generateRandomString(10);

  return (
    <Box sx={{ width: '100%' }}>
      <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
        <Tabs
          value={value}
          onChange={handleChange}
          aria-label='basic tabs example'
        >
          {data.map((_, index) => (
            <Tab
              label={`Result ${index + 1}`}
              key={index}
              {...a11yProps(idPrefix, index)}
            />
          ))}
        </Tabs>
      </Box>

      {data.map((e, index) => {
        return (
          <CustomTabPanel
            idPrefix={idPrefix}
            key={`panel${index}`}
            value={value}
            index={index}
          >
            {e.Err ? (
              <Alert severity='error'>{e.Err}</Alert>
            ) : (
              <QueryResp data={e.Ok} order={''} orderBy={''} reloadSort={function (order: string, orderBy: string): void {
                  throw new Error('Function not implemented.');
                } }></QueryResp>
            )}
          </CustomTabPanel>
        );
      })}
    </Box>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));
export { MultiQueryResp };
root.render(
  <React.StrictMode>
    <QueryResp data={testExampleQueryResp()}></QueryResp>

    {/* <MultiQueryResp data={testExampleMultiQueryResp()}></MultiQueryResp> */}
  </React.StrictMode>
);
