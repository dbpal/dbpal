import { Box, Modal, Stack } from '@mui/material';
import { types } from './ApiStruct.ts';
import { DataGrid, GridColDef, GridPaginationModel } from '@mui/x-data-grid';

import { v4 as uuidv4 } from 'uuid';
import { useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { EscDownEvent, modalStyle, SimpleFlexSizeEditor } from './Common.tsx';
import DoneIcon from '@mui/icons-material/Done';
import CloseIcon from '@mui/icons-material/Close';
import {
  columnWidths,
  gridStyle,
  saveAndSetSize,
  SettingContext,
  VisiableContext,
} from './helper.ts';
import { ModalHeader } from './CommonComp.tsx';
const pageSizeOptions = [10, 15];

const initialState: object = {
  sorting: { sortModel: [{ field: 'time', sort: 'desc' }] },
};

function QueryLog(props: { logs: types.QueryLogEvent[]; uuid: string }) {
  let { logs, uuid: language } = props;
  let columns = [
    {
      field: 'time',
      width: columnWidths.getWidth('time'),
      renderCell: (p) => {
        let t = p.row.time;
        return new Date(t * 1000).toLocaleString();
      },
      filterable: false,
    },
    {
      width: columnWidths.getWidth('success'),
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
  ] as GridColDef[];

  logs.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidv4();
    }
  });

  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPagesize] = useState(10);
  const onRowDoubleClick = useCallback((e) => {
    setSelectSql(e.row.sql);
  }, []);
  const paginationModel = useMemo(() => {
    return { page: page, pageSize: pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useCallback((e: GridPaginationModel) => {
    setPage(e.page);
    setPagesize(e.pageSize);
  }, []);
  let vis = useContext(VisiableContext);
  const setting = useContext(SettingContext);

  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'query-sql');
      return c;
    }
  }, [modalBox]);
  const onColumnResize = useCallback(({ colDef, width }) => {
    let field = colDef.field;
    if (field == 'time' || field == 'success') {
      columnWidths.saveColumnWidths(field, width);
    }
  }, []);

  return (
    <Stack>
      {vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            onRowDoubleClick={onRowDoubleClick}
            columns={columns}
            rows={logs}
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            pageSizeOptions={pageSizeOptions}
            paginationModel={paginationModel}
            onPaginationModelChange={onPaginationModelChange}
            initialState={initialState}
            onColumnResize={onColumnResize}
          ></DataGrid>
        </div>
      )}

      {selectSql && (
        <Modal
          open={!!selectSql}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setSelectSql('');
          }}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setModalBox}
            width="60%"
            height="40vh"
          >
            <ModalHeader title="View SQL"></ModalHeader>
            <SimpleFlexSizeEditor
              language={language}
              value={selectSql}
            ></SimpleFlexSizeEditor>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

export { QueryLog };
