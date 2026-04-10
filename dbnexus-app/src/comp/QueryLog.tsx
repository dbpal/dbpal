import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
  IconButton,
  Modal,
  Paper,
  Stack,
  Tooltip,
} from "@mui/material";
import { DataGrid, GridColDef, GridSlotsComponent } from "@mui/x-data-grid";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  dbpalQuerykey,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
} from "./Common";
import DoneIcon from "@mui/icons-material/Done";
import CloseIcon from "@mui/icons-material/Close";
import {
  dataGridStyle,
  DataSourceKind,
  getDefautLanguage,
  pageSizeOptions,
  saveQueryKey,
  secondConfirm,
} from "./helper.ts";
import { ModalHeader, ResizeParentHeight } from "./CommonComp.tsx";
import { BsFillLightbulbFill } from "react-icons/bs";
import {
  AppDataGridPagination,
  EscPressedDo,
  ListenElementSize,
  SimpleFlexEditor,
  StackRowCenter,
} from "./CommonComp2.tsx";
import {
  useBoolSwitch,
  useColumnWidthStore,
  useNotification,
  useQueryLogStore,
} from "./store.tsx";

const initialState: object = {
  sorting: { sortModel: [{ field: "create_at", sort: "desc" }] },
};

function QueryLog(props: {
  uuid: string;
  showQueryLog: boolean;
  setShowQueryLog: (x: boolean) => void;
  executeTimes: number;
  queryId?: string;
  kind: DataSourceKind;
}) {
  const { uuid, showQueryLog, setShowQueryLog, executeTimes, queryId, kind } = props;
  const [selectSql, setSelectSql] = useState<{ sql: string; language: string }>(null);
  const [logs, setLogs] = useState([]);
  const onRowDoubleClick = useCallback((e) => {
    setSelectSql(e.row);
  }, []);

  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
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
  const columns = [
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
  ] as GridColDef[];

  useEffect(() => {
    const handleReload = async () => {
      setLoading(true);
      try {
        const logs = await query(uuid);
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
  }, [query, showNotification, showQueryLog, uuid, executeTimes]);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      const field = colDef.field;
      if (field == "create_at" || field == "success") {
        setColumnWidth(field, width);
      }
    },
    [setColumnWidth],
  );
  const [modalBox, setModalBox] = useState(null);
  const [dom, setDom] = useState<HTMLDivElement>(null);
  const { getSwitch, setSwitch } = useBoolSwitch();

  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{ flex: 1, justifyContent: "flex-end", overflow: "auto" }}>
            <AppDataGridPagination showPageSelect />
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
            <Button
              onClick={async () => {
                try {
                  const confirm = await secondConfirm({
                    confirmText: "delete",
                    title: "delete all query log?",
                  });
                  if (!confirm) {
                    return;
                  }
                  const rowsAffected = await deleteByUuid(uuid);
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
              delete
            </Button>
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
          onRowDoubleClick={onRowDoubleClick}
          columns={columns}
          rows={logs}
          slots={slots}
          style={dataGridStyle}
          pageSizeOptions={pageSizeOptions}
          initialState={initialState}
          onColumnResize={onColumnResize}
          loading={loading}
        />
      )}

      {dom && (
        <ResizeParentHeight box={dom} min={200} saveHeightName="query-log-height" />
      )}

      <ListenElementSize dom={modalBox} set="both" name="query-log-show-sql" />

      {selectSql && (
        <Modal
          open={!!selectSql}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setSelectSql(null);
          })}
          disableEnforceFocus
          disableAutoFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "60%", height: "40vh" }}
            ref={setModalBox}>
            <ModalHeader
              title="View SQL"
              handleClose={() => {
                setSelectSql(null);
              }}
            />
            <SimpleFlexEditor
              value={selectSql.sql}
              handleEditor={(e) => {
                e.getModel()[dbpalQuerykey] = new NameAndQueryId(uuid, kind, queryId);
              }}
              language={
                selectSql.language ? selectSql.language : getDefautLanguage(kind)
              }
            />
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

export { QueryLog };
