import { Box, Modal, Stack } from '@mui/material';
import { types } from './ApiStruct.ts';
import { DataGrid, GridPaginationModel } from '@mui/x-data-grid';

import { v4 as uuidv4 } from 'uuid';
import { useMemo, useState } from 'react';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  modalStyle,
  SimpleResizeEditor,
} from './Common.tsx';
import DoneIcon from '@mui/icons-material/Done';
import CloseIcon from '@mui/icons-material/Close';
import { GridInitialStateCommunity } from '@mui/x-data-grid/models/gridStateCommunity';

const initialState: GridInitialStateCommunity = {
  sorting: { sortModel: [{ field: 'time', sort: 'desc' }] },
};
function QueryLog(props: { logs: types.QueryLogEvent[]; language: string }) {
  let { logs, language } = props;
  let columns = [
    {
      field: 'time',
      flex: 1,
      renderCell: (p) => {
        let t = p.row.time;
        return new Date(t * 1000).toLocaleString();
      },
      filterable: false,
    },
    {
      field: 'success',
      renderCell: (p) => {
        let s = p.row.success;
        if (s) {
          return <DoneIcon color="success"></DoneIcon>;
        } else {
          return <CloseIcon color="error"></CloseIcon>;
        }
      },
      filterable: false,
    },
    { field: 'sql', flex: 1 },
  ];

  logs.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidv4();
    }
  });

  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPagesize] = useState(10);
  const onRowDoubleClick = useMemo(() => {
    return (e) => {
      setSelectSql(e.row.sql);
    };
  }, []);

  const paginationModel = useMemo(() => {
    return { page: page, pageSize: pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useMemo(() => {
    return (e: GridPaginationModel) => {
      setPage(e.page);
      setPagesize(e.pageSize);
    };
  }, []);

  return (
    <Stack>
      <DataGrid
        onRowDoubleClick={onRowDoubleClick}
        columns={columns}
        rows={logs}
        rowHeight={datagridRowHeight}
        columnHeaderHeight={datagridHeaderHeight}
        pageSizeOptions={[10, 15]}
        paginationModel={paginationModel}
        onPaginationModelChange={onPaginationModelChange}
        initialState={initialState}
      ></DataGrid>

      {selectSql && (
        <Modal
          open={!!selectSql}
          onClose={() => {
            setSelectSql('');
          }}
          aria-labelledby="sql"
        >
          <Box sx={modalStyle} width="800px">
            <SimpleResizeEditor
              language={language}
              value={selectSql}
              minHeigth={200}
            ></SimpleResizeEditor>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

export { QueryLog };
