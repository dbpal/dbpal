'use strict';
import {
  Box,
  Button,
  FormControlLabel,
  IconButton,
  Modal,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
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
import {
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  SetAutoCompletionName,
} from './Common';
import DoneIcon from '@mui/icons-material/Done';
import CloseIcon from '@mui/icons-material/Close';
import {
  ColumnWidth,
  dataGridStyle,
  DataSourceKind,
  getLanguageFromDataSourceKind,
  pageSizeOptions,
  QueryLogHandler,
  saveAndSetSize,
  SavePageSize,
  SaveQueryLogConfigure,
  secondConfirm,
  SettingContext,
} from './helper.ts';
import { ModalHeader, ResizeParentHeight } from './CommonComp.tsx';
import { BsFillLightbulbFill } from 'react-icons/bs';
import { NotificationContext } from './Notification.tsx';
import { SimpleFlexEditor } from './CommonComp2.tsx';

const initialState: object = {
  sorting: { sortModel: [{ field: 'create_at', sort: 'desc' }] },
};

function QueryLog(props: {
  uuid: string;
  showQueryLog: boolean;
  setShowQueryLog: (x: boolean) => void;
  executeTimes: number;
  queryId?: number;
  kind: DataSourceKind;
}) {
  const { uuid, showQueryLog, setShowQueryLog, executeTimes, queryId, kind } =
    props;
  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(0); // datagrid start with 0
  const [pageSize, setPagesize] = useState(SavePageSize.instance.get());
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
  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason) => {
      setPage(e.page);
      setPagesize(e.pageSize);
      if (reason.reason == 'setPaginationModel') {
        SavePageSize.instance.save(e.pageSize);
      }
    },
    []
  );

  let sqlDesc = 'sql';
  switch (kind) {
    case DataSourceKind.Redis:
      sqlDesc = 'command';
      break;
    case DataSourceKind.Mongo:
      sqlDesc = 'code';
      break;
  }
  const { showNotification } = useContext(NotificationContext);
  const handleReload = async () => {
    try {
      const total = await handler.current.queryCount(matchSql);
      setTotal(total);
      const logs = await handler.current.query(
        matchSql,
        page + 1,
        pageSize,
        orderBy
      );
      setLogs(logs);
    } catch (e) {
      showNotification('error', 'get query log error', e);
      return;
    }
  };
  const columns = [
    {
      field: 'create_at',
      width: ColumnWidth.instance.get('create_at'),
      renderHeader: () => {
        return 'time';
      },
      renderCell: (params) => {
        // 加z是utc时间
        return <div>{new Date(params.value + 'Z').toLocaleString()}</div>;
      },
    },
    {
      width: ColumnWidth.instance.get('success'),
      field: 'success',
      renderCell: (p) => {
        const success = p.row.success;
        if (success) {
          return <DoneIcon color="success"></DoneIcon>;
        } else {
          return <CloseIcon color="error"></CloseIcon>;
        }
      },
      filterable: false,
    },
    {
      field: 'sql',
      flex: 1,
      renderHeader: () => {
        return sqlDesc;
      },
    },
  ] as GridColDef[];

  useEffect(() => {
    if (showQueryLog) {
      handleReload();
    }
  }, [page, pageSize, matchSql, orderBy, executeTimes, showQueryLog]);

  const setting = useContext(SettingContext);
  const onColumnResize = useCallback(({ colDef, width }) => {
    const field = colDef.field;
    if (field == 'create_at' || field == 'success') {
      ColumnWidth.instance.save(field, width);
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
      const clean = saveAndSetSize(modalBox, 'query-sql');
      return clean;
    }
  }, [modalBox]);
  const [dom, setDom] = useState<HTMLDivElement>(null);
  const [saveQueryLog, setSaveQueryLog] = useState(SaveQueryLogConfigure.get());
  return (
    <Paper
      style={{
        position: 'absolute',
        right: 0,
        top: 0,
        zIndex: 13,
        width: '75%',
        display: showQueryLog ? 'flex' : 'none',
        flexDirection: 'column',
        height: '400px',
        maxHeight: '80vh',
      }}
      hidden={true}
      ref={setDom}
      elevation={16}
    >
      <Stack
        direction="row"
        style={{
          alignItems: 'center',
          padding: '0px 4px',
          justifyContent: 'space-between',
        }}
      >
        <Stack direction="row" style={{ alignItems: 'center' }}>
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
        </Stack>
        <Stack direction="row" style={{ alignItems: 'center' }}>
          filter {sqlDesc}:
          <TextField
            value={matchSql}
            onChange={(e) => {
              setMatchSql(e.target.value);
            }}
            style={{ flex: 1, maxWidth: '20rem' }}
          ></TextField>
          {total > 0 && (
            <Button
              onClick={async () => {
                try {
                  const confirm = await secondConfirm({
                    confirmText: 'delete',
                    title: 'delete all query log?',
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
            >
              delete
            </Button>
          )}
          <Tooltip
            title={`auto save query log to local database maybe danger.`}
          >
            <FormControlLabel
              control={
                <Switch
                  checked={saveQueryLog}
                  onChange={() => {
                    let newValue = !saveQueryLog;
                    SaveQueryLogConfigure.set(newValue);
                    setSaveQueryLog(newValue);
                  }}
                />
              }
              label="save query log"
            />
          </Tooltip>
        </Stack>
      </Stack>

      <DataGrid
        showToolbar
        showCellVerticalBorder
        showColumnVerticalBorder
        onRowDoubleClick={onRowDoubleClick}
        columns={columns}
        rows={logs}
        style={dataGridStyle}
        rowHeight={setting.appearance.datagridRowHeight()}
        columnHeaderHeight={setting.appearance.datagridRowHeight()}
        pageSizeOptions={pageSizeOptions}
        paginationModel={paginationModel}
        onPaginationModelChange={onPaginationModelChange}
        initialState={initialState}
        onColumnResize={onColumnResize}
        onSortModelChange={onSortModelChange}
        rowCount={total}
        paginationMode="server"
      ></DataGrid>

      {dom && (
        <ResizeParentHeight
          box={dom}
          min={200}
          saveHeightName="query-log-height"
        ></ResizeParentHeight>
      )}

      {selectSql && (
        <Modal
          open={!!selectSql}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectSql('');
          })}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setModalBox}
            width="60%"
            height="40vh"
          >
            <ModalHeader
              title="View SQL"
              handleClose={() => {
                setSelectSql('');
              }}
            ></ModalHeader>
            <SimpleFlexEditor
              value={selectSql}
              handleEditor={(e) => {
                SetAutoCompletionName.set(
                  e.getModel().id,
                  new NameAndQueryId(uuid, kind, queryId)
                );
              }}
              language={getLanguageFromDataSourceKind(kind)}
            ></SimpleFlexEditor>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

export { QueryLog };
