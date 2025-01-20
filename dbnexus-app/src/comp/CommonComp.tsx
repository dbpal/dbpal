import { FormControl, InputLabel, Select, MenuItem } from '@mui/material';
import { ReactNode } from 'react';
import { types } from './ApiStruct';
import { DataGrid } from '@mui/x-data-grid';
const LogLevelSel = ({ handleChange, level }) => {
  let levels = ['trace', 'debug', 'info', 'warn', 'error'];

  return (
    <FormControl style={{ width: '100px' }}>
      <InputLabel id="dropdown-label">select a level</InputLabel>
      <Select
        labelId="dropdown-label"
        value={level}
        onChange={(e) => {
          handleChange(e.target.value);
        }}
        variant="outlined"
        size="small"
      >
        {levels.map((e) => (
          <MenuItem value={e} key={e}>
            {e}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};

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

const datagridRowHeight = 25;
const datagridHeaderHeight = 36;

function StickyNav(props: { children: ReactNode[] | ReactNode }) {
  const { children } = props;

  return (
    <div style={{ position: 'sticky', top: 0, border: '1px blue solid' }}>
      {children}
    </div>
  );
}

export {
  LogLevelSel,
  StickyNav,
  MetaValuesView,
  datagridRowHeight,
  datagridHeaderHeight,
};
