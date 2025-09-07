import {
  Box,
  Button,
  IconButton,
  Modal,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import {
  DataGrid,
  GridColDef,
  GridPaginationModel,
  GridSortModel,
} from '@mui/x-data-grid';
import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { EscDownEvent, modalStyle, SimpleFlexEditor } from './Common.tsx';
import DoneIcon from '@mui/icons-material/Done';
import CloseIcon from '@mui/icons-material/Close';
import {
  columnWidths,
  gridStyle,
  QueryLogHandler,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
} from './helper.ts';
import { ModalHeader } from './CommonComp.tsx';
import { BsFillLightbulbFill } from 'react-icons/bs';
import { NotificationContext } from './Notification.tsx';
const pageSizeOptions = [10, 15];

const initialState: object = {
  sorting: { sortModel: [{ field: 'create_at', sort: 'desc' }] },
};

function QueryLog(props: {
  uuid: string;
  showQueryLog;
  setShowQueryLog;
  executeTimes;
}) {
  let { uuid, showQueryLog, setShowQueryLog, executeTimes } = props;
  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(0); // datagrid start with 0
  const [pageSize, setPagesize] = useState(10);
  const [logs, setLogs] = useState([]);
  const [matchSql, setMatchSql] = useState('');
  const [orderBy, setOrderBy] = useState('');
  const [total, setTotal] = useState(0);
  const onRowDoubleClick = useCallback((e) => {
    setSelectSql(e.row.sql);
  }, []);
  const handler = useRef(new QueryLogHandler(uuid));
  const paginationModel = useMemo(() => {
    return { page: page, pageSize: pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useCallback((e: GridPaginationModel) => {
    setPage(e.page);
    setPagesize(e.pageSize);
  }, []);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = async () => {
    try {
      let total = await handler.current.queryCount(matchSql);
      setTotal(total);
      let x = await handler.current.query(
        matchSql,
        page + 1,
        pageSize,
        orderBy
      );
      setLogs(x);
    } catch (e) {
      showNotification('error', 'get query log error', e);
      return;
    }
  };
  const columns = [
    {
      field: 'create_at',
      width: columnWidths.getWidth('create_at'),
      renderHeader: () => {
        return 'time';
      },
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

  useEffect(() => {
    if (showQueryLog) {
      handleReload();
    }
  }, [page, pageSize, matchSql, orderBy, executeTimes, showQueryLog]);

  const setting = useContext(SettingContext);
  const onColumnResize = useCallback(({ colDef, width }) => {
    let field = colDef.field;
    if (field == 'create_at' || field == 'success') {
      columnWidths.saveColumnWidths(field, width);
    }
  }, []);
  const onSortModelChange = useCallback((modal: GridSortModel) => {
    modal = modal.filter((x) => x.sort);
    const orderBy = modal.map((e) => `${e.field} ${e.sort}`).join(' ');
    setOrderBy(orderBy);
  }, []);
  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'query-sql');
      return c;
    }
  }, [modalBox]);

  return (
    <Paper
      style={{
        position: 'absolute',
        right: 0,
        top: 0,
        zIndex: 13,
        width: '75%',
        display: showQueryLog ? 'block' : 'none',
      }}
      elevation={16}
    >
      <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
        <IconButton
          onClick={() => {
            setShowQueryLog(false);
          }}
        >
          <CloseIcon></CloseIcon>
        </IconButton>
        <Tooltip title="double click to open code editor">
          <BsFillLightbulbFill />
        </Tooltip>
        <Tooltip title="esc can close query log too">
          <span>esc</span>
        </Tooltip>
        <div style={{ width: '20%' }}></div>
        <Typography color="info">match sql: </Typography>
        <TextField
          size="small"
          value={matchSql}
          onChange={(e) => {
            setMatchSql(e.target.value);
          }}
          style={{ flex: 1 }}
        ></TextField>
        {total > 0 && (
          <Button
            onClick={async () => {
              try {
                const confirm = await secondConfirm({
                  confirmText: 'delete',
                  title: 'delete all qyery log?',
                });
                if (!confirm) {
                  return;
                }
                const r = await handler.current.delete();
                showNotification(
                  'success',
                  `deleted ${r.rowsAffected} query log`
                );
                setShowQueryLog(false);
              } catch (e) {
                showNotification('error', 'delete failed', e);
                return;
              }
            }}
            size="small"
            variant="contained"
          >
            delete
          </Button>
        )}
      </Stack>
      <Stack>
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            showToolbar
            style={gridStyle}
            showCellVerticalBorder
            showColumnVerticalBorder
            onRowDoubleClick={onRowDoubleClick}
            columns={columns}
            rows={logs}
            rowHeight={setting.datagridRowHeightXXX()}
            columnHeaderHeight={setting.datagridRowHeightXXX()}
            pageSizeOptions={pageSizeOptions}
            paginationModel={paginationModel}
            onPaginationModelChange={onPaginationModelChange}
            initialState={initialState}
            onColumnResize={onColumnResize}
            onSortModelChange={onSortModelChange}
            rowCount={total}
            paginationMode="server"
          ></DataGrid>
        </div>

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
              <SimpleFlexEditor
                language={uuid}
                value={selectSql}
              ></SimpleFlexEditor>
            </Box>
          </Modal>
        )}
      </Stack>
    </Paper>
  );
}

export { QueryLog };
