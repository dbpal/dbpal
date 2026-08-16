import {
  AlertTitle,
  Checkbox,
  Dialog,
  FormControlLabel,
  IconButton,
  Paper,
  Stack,
  Tooltip,
} from "@mui/material";
import {
  Dispatch,
  SetStateAction,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { EscDownEvent, modalHandleClose, NameAndQueryId } from "./Common";
import DoneIcon from "@mui/icons-material/Done";
import CloseIcon from "@mui/icons-material/Close";
import {
  createMongoModel,
  dataGridStyle,
  DataSourceKind,
  getDatabaseKindLanguage,
  pageSizeOptions,
  saveQueryKey,
  secondConfirm,
  DbnameContext,
} from "./helper.ts";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { ResizeParentHeight } from "./CommonComp.tsx";
import { BsFillLightbulbFill } from "react-icons/bs";
import {
  AppDataGridPagination,
  EscPressedDo,
  SimpleFlexEditor,
  StackRowCenter,
} from "./CommonComp2.tsx";
import {
  useBoolSwitch,
  useColumnWidthStore,
  useNotification,
  useQueryLogStore,
} from "./store.tsx";
import { CustomToolbar } from "./CustomToolBar.tsx";
import { DataGrid, GridColDef, GridSlotsComponent } from "./datagridExport.ts";

function QueryLog(props: {
  showQueryLog: boolean;
  setShowQueryLog: Dispatch<SetStateAction<boolean>>;
  queryId?: string;
}) {
  const { dbName, kind } = useContext(DbnameContext);
  const { showQueryLog, setShowQueryLog, queryId } = props;
  const [selectSql, setSelectSql] = useState<string>("");
  const [logs, setLogs] = useState([]);
  const { getColumnWidth } = useColumnWidthStore();
  const { query, deleteByUuid } = useQueryLogStore();

  const [loading, setLoading] = useState(false);
  let sqlDesc = "sql";
  switch (kind) {
    case DataSourceKind.Redis:
      sqlDesc = "command";
      break;
    case DataSourceKind.Mongo:
      sqlDesc = "code";
      break;
  }
  const { showNotification } = useNotification();
  const columns: GridColDef[] = [
    {
      field: "create_at",
      width: getColumnWidth("create_at"),
      renderHeader: () => {
        return "time";
      },
      renderCell: (params) => {
        // 加z是utc时间
        return <div>{new Date(params.value).toLocaleString()}</div>;
      },
    },
    {
      width: getColumnWidth("success"),
      field: "success",
      renderCell: (p) => {
        const success = p.row.success;
        if (success) {
          return <DoneIcon color="success" />;
        } else {
          return <CloseIcon color="error" />;
        }
      },
      filterable: false,
    },
    {
      field: "sql",
      flex: 1,
      renderHeader: () => {
        return sqlDesc;
      },
    },
  ];

  useEffect(() => {
    const handleReload = async () => {
      setLoading(true);
      try {
        const logs = await query(dbName);
        setLogs(logs);
      } catch (e) {
        showNotification({
          severity: "error",
          title: "get query log error",
          message: e,
        });
        return;
      } finally {
        setLoading(false);
      }
    };
    if (showQueryLog) {
      handleReload();
    }
  }, [query, showNotification, showQueryLog, dbName]);

  const [dom, setDom] = useState<HTMLDivElement>(null);
  const { getSwitch, setSwitch } = useBoolSwitch();

  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              flex: 1,
              justifyContent: "space-between",
              overflow: "auto",
              alignItems: "center",
            }}>
            <CustomToolbar />
            <Stack direction="row" style={{ alignContent: "center" }}>
              <AppDataGridPagination showPageSelect />
            </Stack>
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, []);

  return (
    <Paper
      style={{
        position: "absolute",
        right: 0,
        top: 0,
        zIndex: 13,
        width: "75%",
        display: showQueryLog ? "flex" : "none",
        flexDirection: "column",
        height: "400px",
        maxHeight: "80vh",
      }}
      ref={setDom}
      elevation={16}>
      <EscPressedDo
        doingIt={() => {
          return showQueryLog;
        }}
        setUndo={() => {
          setShowQueryLog(false);
        }}
      />
      <Stack
        direction="row"
        style={{
          alignItems: "center",
          padding: "0px 4px",
          justifyContent: "space-between",
        }}>
        <StackRowCenter>
          <IconButton
            onClick={() => {
              setShowQueryLog(false);
            }}>
            <CloseIcon />
          </IconButton>
          <Tooltip title="double click to open code editor">
            <BsFillLightbulbFill />
          </Tooltip>
          <Tooltip title="esc can close query log too">
            <span>esc</span>
          </Tooltip>
        </StackRowCenter>
        <StackRowCenter>
          {logs.length > 0 && (
            <IconButton
              onClick={async () => {
                try {
                  const confirm = await secondConfirm({
                    confirmText: "delete",
                    title: "delete all query log?",
                  });
                  if (!confirm) {
                    return;
                  }
                  const rowsAffected = await deleteByUuid(dbName);
                  showNotification({
                    severity: "success",
                    title: `deleted ${rowsAffected} query log`,
                  });
                  setShowQueryLog(false);
                } catch (e) {
                  showNotification({
                    severity: "error",
                    title: "delete failed",
                    message: e,
                  });
                  return;
                }
              }}>
              <DeleteForeverIcon color="warning" />
            </IconButton>
          )}
          <Tooltip title={`auto save query log to local database maybe danger.`}>
            <FormControlLabel
              control={
                <Checkbox
                  checked={getSwitch(saveQueryKey)}
                  onChange={() => {
                    setSwitch(saveQueryKey, !getSwitch(saveQueryKey));
                  }}
                />
              }
              label="save query log"
            />
          </Tooltip>
        </StackRowCenter>
      </Stack>
      {showQueryLog && logs.length > 0 && (
        <DataGrid
          onRowDoubleClick={(e) => {
            setSelectSql(e.row.sql);
          }}
          showToolbar={false}
          columns={columns}
          rows={logs}
          slots={slots}
          style={dataGridStyle}
          pageSizeOptions={pageSizeOptions}
          initialState={{
            sorting: { sortModel: [{ field: "create_at", sort: "desc" }] },
          }}
          loading={loading}
        />
      )}
      {dom && (
        <ResizeParentHeight box={dom} min={200} saveHeightName="query-log-height" />
      )}
      {selectSql && (
        <Dialog
          fullWidth
          maxWidth="md"
          slotProps={{ paper: { style: { height: "50vh" } } }}
          open={true}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectSql(null);
          })}
          disableEnforceFocus
          disableAutoFocus>
          <AlertTitle> view sql </AlertTitle>
          <SimpleFlexEditor
            value={selectSql}
            handleEditor={(ed) => {
              if (kind == DataSourceKind.Mongo) {
                const model = createMongoModel(`//@ts-ignore
const { db, FS, HTTP, BSON } = mongoHandle;
`);
                ed.setModel(model);
              }
              ed.focus();
              ed.getModel()._dbpal_query_id = new NameAndQueryId(dbName, kind, queryId);
            }}
            language={getDatabaseKindLanguage(kind)}
          />
        </Dialog>
      )}
    </Paper>
  );
}

export { QueryLog };
