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
  dataGridParentStyle,
  pageSizeOptions,
  QueryLogHandler,
  saveAndSetSize,
  SavePageSize,
  secondConfirm,
  SettingContext,
  UserSetting,
} from './helper.ts';
import { ModalHeader, ResizeParentHeight } from './CommonComp.tsx';
import { BsFillLightbulbFill } from 'react-icons/bs';
import { NotificationContext } from './Notification.tsx';
import { SimpleFlexEditor } from './CommonComp2.tsx';

const initialState: object = {
  sorting: { sortModel: [{ field: 'create_at', sort: 'desc' }] },
};

enum QueryLogKind {
  Sql,
  Redis,
  Mongo,
}
function QueryLog(props: {
  uuid: string;
  showQueryLog: boolean;
  setShowQueryLog: (x: boolean) => void;
  executeTimes: number;
  language: string;
  queryId?: number;
  kind: QueryLogKind;
}) {
  let {
    uuid,
    showQueryLog,
    setShowQueryLog,
    executeTimes,
    language,
    queryId,
    kind,
  } = props;
  const [selectSql, setSelectSql] = useState('');
  const [page, setPage] = useState(0); // datagrid start with 0
  const [pageSize, setPagesize] = useState(SavePageSize.instance.get());
  const [logs, setLogs] = useState([]);
  const [loaded, setLoaded] = useState(false);
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
    case QueryLogKind.Redis:
      sqlDesc = 'command';
      break;
    case QueryLogKind.Mongo:
      sqlDesc = 'code';
      break;
  }
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
      setLoaded(true);
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
        let s = p.row.success;
        if (s) {
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
    let field = colDef.field;
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
      let c = saveAndSetSize(modalBox, 'query-sql');
      return c;
    }
  }, [modalBox]);
  const [dom, setDom] = useState<HTMLDivElement>(null);

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
        style={{ alignItems: 'center', padding: '0px 4px' }}
        spacing={1}
      >
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
        <Tooltip title={`filter ${sqlDesc}`}>
          <TextField
            value={matchSql}
            onChange={(e) => {
              setMatchSql(e.target.value);
            }}
            style={{ flex: 1 }}
          ></TextField>
        </Tooltip>
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
        <Tooltip title={`auto save query log to local database maybe danger.`}>
          <FormControlLabel
            control={
              <Switch
                checked={setting.saveQueryLog}
                onChange={() => {
                  UserSetting.set({
                    saveQueryLog: !setting.saveQueryLog,
                  } as UserSetting).catch((e) => {
                    showNotification(
                      'error',
                      'save auto save query log error',
                      e
                    );
                  });
                }}
              />
            }
            label="save query log"
          />
        </Tooltip>
      </Stack>

      {loaded && (
        <div style={{ ...dataGridParentStyle }}>
          <DataGrid
            showToolbar
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
      )}
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
            <ModalHeader title="View SQL"></ModalHeader>
            <SimpleFlexEditor
              language={language}
              value={selectSql}
              handleEditor={(e) => {
                SetAutoCompletionName.set(
                  e.getModel().id,
                  new NameAndQueryId(uuid, queryId)
                );
              }}
            ></SimpleFlexEditor>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

export { QueryLog, QueryLogKind };
