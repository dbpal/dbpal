import { Box, Modal, Stack } from '@mui/material';
import { types } from './ApiStruct.ts';
import { DataGrid } from '@mui/x-data-grid';

import { v4 as uuidv4 } from 'uuid';
import { useContext, useState } from 'react';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  modalStyle,
  SettingsContext,
} from './Common.tsx';
import { Editor } from '@monaco-editor/react';
import DoneIcon from '@mui/icons-material/Done';
import CloseIcon from '@mui/icons-material/Close';
function QueryLog(props: { logs: types.QueryLogEvent[]; language: string }) {
  let { logs, language } = props;
  let columns = [
    {
      field: 'time',
      flex: 1,
      renderCell: (p) => {
        let t = p.row.time;
        return new Date(t).toLocaleString();
      },
      filterable: false,
    },
    {
      field: 'success',
      renderCell: (p) => {
        let s = p.row.success;
        if (s) {
          return <DoneIcon color='success'></DoneIcon>;
        } else {
          return <CloseIcon color='error'></CloseIcon>;
        }
      },
      filterable: false,
    },
    { field: 'sql', flex: 1 },
  ];

  logs.forEach((e) => {
    if (!('id' in e)) {
      // @ts-ignore
      e.id = uuidv4();
    }
  });

  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPagesize] = useState(
    useContext(SettingsContext).page_size
  );
  return (
    <Stack>
      <DataGrid
        onRowDoubleClick={(e) => {
          setSelectSql(e.row.sql);
        }}
        getRowId={(e) => e['time']}
        columns={columns}
        rows={logs}
        rowHeight={datagridRowHeight}
        columnHeaderHeight={datagridHeaderHeight}
        pageSizeOptions={[10, 15]}
        paginationModel={{ page: page, pageSize: pageSize }}
        onPaginationModelChange={(e) => {
          setPage(e.page);
          setPagesize(e.pageSize);
        }}
        initialState={{
          sorting: { sortModel: [{ field: 'time', sort: 'desc' }] },
        }}
      ></DataGrid>

      {selectSql && (
        <Modal
          open={!!selectSql}
          onClose={() => {
            setSelectSql('');
          }}
          aria-labelledby='sql'
        >
          <Stack sx={modalStyle} width='800px'>
            <Box>
              <Editor
                language={language}
                height='300px'
                value={selectSql}
                onMount={}
              ></Editor>
            </Box>
          </Stack>
        </Modal>
      )}
    </Stack>
  );
}

export { QueryLog };
