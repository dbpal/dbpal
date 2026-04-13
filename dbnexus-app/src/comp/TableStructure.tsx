import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  DefAndMetas,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  unwrapCommand,
} from "./Common";
import {
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  secondConfirm,
} from "./helper";
import { ClearableTextField } from "./CommonComp2";
import CancelIcon from "@mui/icons-material/Cancel";
import { invoke } from "@tauri-apps/api/core";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "@mui/x-data-grid";
import {
  Autocomplete,
  Box,
  Button,
  ButtonGroup,
  Checkbox,
  Divider,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Radio,
  RadioGroup,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import EditIcon from "@mui/icons-material/Edit";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import AddIcon from "@mui/icons-material/Add";
import { BSON } from "bson";
import { MongoValueEditor } from "./MongoDB";
import { ModalHeader, TrueOrFalse } from "./CommonComp";
import { DefAndMetasView, TriggerDetailView } from "./DefAndMetaView";
import { formatCreateTableAndIndexSql } from "./handlers";
import {
  AppDataGridPagination,
  ColumnNullness,
  CopySql,
  CreateIndexView,
  ListenElementSize,
  SqlDefaultValue,
  StackRowCenter,
  TypeSelect,
} from "./CommonComp2";
import {
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  useUserSettings,
} from "./store";
import {
  ColumnType,
  commands,
  CreateIndexReq,
  Field,
  Index,
  SqlNullAble,
  TriggerDetail,
  TriggerEnable,
} from "./bindings";

function ColumnMenu(props: {
  col: string;
  content: string;
  supportModifyColumn;
  handleDrop;
}) {
  const { showNotification } = useNotification();
  const { col, content, supportModifyColumn, handleDrop } = props;
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}
      {supportModifyColumn && (
        <MenuItem
          onClick={() => {
            handleDrop(col);
          }}>
          <ListItemIcon>
            <DeleteForeverIcon />
          </ListItemIcon>
          <ListItemText> drop </ListItemText>
        </MenuItem>
      )}
    </MenuList>
  );
}

function ColumnsStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useUserSettings();
  const [loading, setLoading] = useState(false);
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [row, setRow] = useState<Field>(null);
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
  const supportModifyColumn = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.SqlServer,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Oracle,
    DataSourceKind.TiDB,
    DataSourceKind.Cassandra,
    DataSourceKind.ClickHouse,
    DataSourceKind.DuckDB,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
  ]).has(kind);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

  const handleReload = useCallback(() => {
    setLoading(true);

    commands
      .showTableColumns(uuid, table)
      .then((ret) => {
        if (ret.status == "ok") {
          setFields(ret.data);
        } else {
          showNotification({
            severity: "error",
            title: "list column failed",
            message: ret.error,
          });
        }
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);

  const handleDrop = useCallback(
    async (col: string) => {
      const confirm = await secondConfirm({
        title: `Drop Column '${col}'`,
        content: "",
        confirmText: "DROP COLUMN",
        kind: "error",
      });
      if (!confirm) {
        return;
      }
      unwrapCommand(commands.dropTableColumn(uuid, table, col))
        .then((sql) => {
          showNotification({
            severity: "success",
            title: "Drop column OK",
            message: <CopySql sql={sql} />,
          });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "Drop column Failure",
            message: e,
          });
        });
    },
    [handleReload, showNotification, table, uuid],
  );

  const [showAdd, setShowAdd] = useState(false);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [
      { field: "field" },
      {
        field: "data_type",
        renderHeader: () => {
          return "type";
        },
      },
      {
        field: "nullable",
        renderCell: (row) => {
          return <TrueOrFalse value={row.row["nullable"]} />;
        },
      },
      {
        field: "is_primary",
        renderHeader: () => {
          return "primary key";
        },
        renderCell: (row) => {
          return <TrueOrFalse value={row.row["is_primary"]} />;
        },
      },
      {
        field: "is_auto_increment",
        renderHeader: () => {
          return "auto increment";
        },
        renderCell: (row) => {
          return <TrueOrFalse value={row.row["is_auto_increment"]} />;
        },
      },
      {
        field: "default_value",
        renderHeader: () => {
          return "default value";
        },
      },
      {
        field: "sortable",
        renderCell: (row) => {
          return <TrueOrFalse value={row.row["sortable"]} />;
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: (
              <ColumnMenu
                supportModifyColumn={supportModifyColumn}
                col={id}
                content={content}
                handleDrop={handleDrop}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export_.csvDelimiter,
          includeHeaders: setting.export_.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    setUpMenu,
    setting.export_.csvDelimiter,
    setting.export_.includeHeader,
    supportModifyColumn,
  ]);

  const getRowId = useCallback((e) => {
    return e["field"];
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    setRow(row.row);
  }, []);

  const [modalBox, setModalBox] = useState(null);

  useEffect(() => {
    if (row) {
      if ([DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind)) {
        showNotification({
          severity: "warning",
          title: `${kind} not support modify column`,
        });
      }
    }
  }, [kind, row, showNotification]);
  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" style={{ flex: 1, justifyContent: "space-between" }}>
            <Stack direction="row">
              {supportModifyColumn && (
                <Tooltip title="double click a row to modify.">
                  <Typography color="info"> modify </Typography>
                </Tooltip>
              )}
            </Stack>
            <AppDataGridPagination />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [supportModifyColumn]);

  return (
    <Stack style={{ height: "100%" }}>
      <StackRowCenter>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        {supportModifyColumn && (
          <Button
            onClick={() => {
              setShowAdd(true);
            }}>
            <AddIcon />
          </Button>
        )}
      </StackRowCenter>

      <DataGrid
        slots={slots}
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        columns={cols}
        rows={fields}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
        loading={loading}
      />

      <ListenElementSize dom={modalBox} name="table-add-column" set="both" />

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <Box
            sx={{ resize: "horizontal", ...modalStyle, width: "60%" }}
            ref={setModalBox}>
            <ModalHeader
              handleClose={() => {
                setShowAdd(false);
              }}
              title="Add Column"
            />
            <TableAddColumn
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
              kind={kind}
              fields={fields}
            />
          </Box>
        </Modal>
      )}
      {supportModifyColumn &&
        row &&
        ![DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind) && (
          <Modal
            open={!!row}
            onClose={modalHandleClose((e: EscDownEvent) => {
              e.preventDefault();
              setRow(null);
            })}>
            <Box
              sx={{ resize: "horizontal", ...modalStyle, width: "60%" }}
              ref={setModalBox}>
              <ModalHeader
                title="Modify Column"
                handleClose={() => {
                  setRow(null);
                }}
              />
              <ModifyTableColumn
                table={table}
                uuid={uuid}
                handleClose={() => {
                  setRow(null);
                }}
                col={row.field}
                typ={row.data_type}
                reload={handleReload}
                kind={kind}
                defaultValue={row.default_value}
              />
            </Box>
          </Modal>
        )}
    </Stack>
  );
}

function RenameIndex(props: {
  uuid: string;
  table: string;
  reload: () => void;
  handleClose: () => void;
  indexName: string;
}) {
  const { uuid, table, reload, handleClose, indexName } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewname] = useState(indexName);
  const { showNotification } = useNotification();
  const handleRename = () => {
    if (newName == indexName) {
      showNotification({ severity: "warning", title: "name not change" });
      return;
    }
    setLoading(true);

    unwrapCommand(commands.renameIndex(uuid, table, indexName, newName))
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Rename OK",
          message: <CopySql sql={sql} />,
        });
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Rename Failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack>
      <TextField
        label="new name"
        value={newName}
        onChange={(e) => {
          setNewname(e.target.value);
        }}
      />
      <Stack direction="row">
        <Button color="primary" loading={loading} onClick={handleRename}>
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function IndexMenu(props: {
  content: string;
  indexName: string;
  setRenameIndexName;
  handleDrop;
}) {
  const { content, indexName, setRenameIndexName, handleDrop } = props;
  const { showNotification } = useNotification();

  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}
      <MenuItem
        onClick={() => {
          setRenameIndexName(indexName);
        }}>
        <ListItemIcon>
          <EditIcon />
        </ListItemIcon>
        <ListItemText> rename </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          handleDrop(indexName);
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> drop </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function IndexesStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useUserSettings();
  const [loading, setLoading] = useState(false);
  const [indexes, setIndexes] = useState<Index[]>([]);
  const { showNotification } = useNotification();
  const [renameIndexName, setRenameIndexName] = useState("");
  const [showAdd, setShowAdd] = useState(false);

  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

  const handleReload = useCallback(() => {
    setLoading(true);

    commands
      .showIndexes(uuid, table)
      .then((ret) => {
        if (ret.status == "ok") {
          setIndexes(ret.data);
        } else {
          showNotification({
            severity: "error",
            title: "show indexes failed",
            message: ret.error,
          });
        }
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);

  const handleDrop = useCallback(
    async (indexName) => {
      const confirm = await secondConfirm({
        title: `drop index '${indexName}'`,
        content: "Are u sure to drop the index",
        confirmText: "Drop",
      });
      if (!confirm) {
        return;
      }
      unwrapCommand(commands.dropIndex(uuid, table, indexName))
        .then((sql) => {
          showNotification({
            severity: "success",
            title: "Drop OK",
            message: <CopySql sql={sql} />,
          });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "Drop index failure",
            message: e,
          });
        });
    },
    [handleReload, showNotification, table, uuid],
  );
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [{ field: "name" }, { field: "kind" }, { field: "columns" }] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          {
            const { field } = extractFieldAndIDFromMouseEvent(e);
            if (!field) {
              return;
            }
          }
          const content = (target as HTMLDivElement).innerHTML;
          const indexName = (currentTarget as HTMLDivElement).getAttribute("data-id");
          setUpMenu({
            content: (
              <IndexMenu
                indexName={indexName}
                setRenameIndexName={setRenameIndexName}
                handleDrop={handleDrop}
                content={content}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export_.csvDelimiter,
          includeHeaders: setting.export_.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    setUpMenu,
    setting.export_.csvDelimiter,
    setting.export_.includeHeader,
  ]);

  const getRowId = useCallback((e) => {
    return e["name"];
  }, []);

  const [modalBox, setModalBox] = useState(null);

  const slots = useMemo(() => {
    return {
      pagination: AppDataGridPagination,
    } as Partial<GridSlotsComponent>;
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row">
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        <Button
          onClick={() => {
            setShowAdd(true);
          }}>
          <AddIcon />
        </Button>
      </Stack>

      <DataGrid
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        columns={cols}
        rows={indexes}
        style={dataGridStyle}
        slots={slots}
        loading={loading}
      />

      {showAdd && kind == DataSourceKind.Mongo && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <Box
            sx={{
              resize: "both",
              ...modalStyle,
              width: "60%",
              height: kind == DataSourceKind.Mongo ? "40vh" : "auto",
            }}
            ref={setModalBox}>
            <ModalHeader
              title="create index"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <MongoAddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
            />
          </Box>
        </Modal>
      )}
      <ListenElementSize dom={modalBox} name="add-index" set="both" />
      {showAdd && kind != DataSourceKind.Mongo && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <Box
            sx={{
              resize: "horizontal",
              ...modalStyle,
              width: "60%",
            }}
            ref={setModalBox}>
            <ModalHeader
              title="create index"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <AddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              kind={kind}
              reload={handleReload}
            />
          </Box>
        </Modal>
      )}

      {renameIndexName && (
        <Modal
          open={!!renameIndexName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRenameIndexName("");
          })}>
          <Box sx={{ ...modalStyle, width: "40%" }}>
            <ModalHeader
              title={`Rename Index`}
              handleClose={() => {
                setRenameIndexName("");
              }}
            />
            <RenameIndex
              uuid={uuid}
              table={table}
              indexName={renameIndexName}
              handleClose={() => {
                setRenameIndexName("");
              }}
              reload={handleReload}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function MongoAddIndex(props: { uuid; table; handleClose; reload }) {
  const { uuid, table, handleClose, reload } = props;

  const indexTypes = ["normal", "unique"];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState("");
  const indexNameDom = useRef<HTMLInputElement>(null);
  const { showNotification } = useNotification();

  const handleAdd = async (keys) => {
    try {
      if (!indexName) {
        showNotification({ severity: "warning", title: "index name can't be none" });
        indexNameDom.current.focus();
        return;
      }
      await unwrapCommand(
        commands.mongoCreateIndex(
          uuid,
          table,
          indexName ? indexName : null,
          BSON.serialize(keys),
          indexType == "unique",
        ),
      );
      showNotification({ severity: "success", title: "create index ok" });
      handleClose();
      reload();
    } catch (e) {
      showNotification({ severity: "error", title: "create index failed", message: e });
    }
  };

  return (
    <Stack style={{ flex: 1 }}>
      <TextField
        value={indexName}
        onChange={(e) => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        required
      />

      <RadioGroup
        row
        onChange={(e) => {
          setIndexType(e.target.value);
        }}
        value={indexType}>
        {indexTypes.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>
      <MongoValueEditor
        setOP={async (doc) => {
          handleAdd(doc);
        }}
        doc={{}}
        setName="create index"
        needConfirm={false}
      />
    </Stack>
  );
}

function AddIndex(props: {
  uuid: string;
  table: string;
  handleClose: () => void;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { uuid, table, handleClose, reload, kind } = props;
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [req, setReq] = useState<CreateIndexReq>(null);
  useEffect(() => {
    commands.showTableColumns(uuid, table).then((ret) => {
      if (ret.status == "ok") {
        setFields(ret.data);
      } else {
        showNotification({
          severity: "error",
          title: "list fields failed",
          message: ret.error,
        });
      }
    });
  }, [showNotification, table, uuid]);
  const handleAdd = async () => {
    if (!req.name) {
      showNotification({ severity: "warning", title: "index name can't be none" });
      return;
    }
    if (req.columns.length == 0) {
      showNotification({ severity: "warning", title: "select at least one column" });
      return;
    }
    try {
      const sql = await unwrapCommand(commands.createIndex(uuid, table, req));
      showNotification({
        severity: "success",
        title: "create index ok",
        message: <CopySql sql={sql} />,
      });
      handleClose();
      reload();
    } catch (e) {
      showNotification({ severity: "error", title: "create index failed", message: e });
    }
  };

  return (
    <Stack>
      <CreateIndexView
        handleReq={(s) => {
          setReq(s);
        }}
        fields={fields.map((e) => e.field)}
        kind={kind}
      />
      <Divider />
      <Stack direction="row">
        <Button onClick={handleAdd} color="primary">
          create
        </Button>
      </Stack>
    </Stack>
  );
}

function TableAddColumn(props: {
  table: string;
  uuid;
  handleClose;
  reload;
  kind: DataSourceKind;
  fields: Field[];
}) {
  const { table, uuid, handleClose, reload, kind, fields } = props;
  const [name, setName] = useState("");
  const [nullable, setNullness] = useState<SqlNullAble>("none");
  const [typ, setTyp] = useState("");
  const { showNotification } = useNotification();
  const [after, setAfter] = useState("");
  const [defaultValue, setDefaultValue] = useState("");
  const supportAfter = [
    DataSourceKind.Mysql,
    kind == DataSourceKind.MariaDB,
    kind == DataSourceKind.TiDB,
  ].includes(kind);
  const nameDom = useRef(null);
  const [loading, setLoading] = useState(false);

  const handleAdd = () => {
    if (!name) {
      showNotification({ severity: "error", title: "column name is empty" });
      nameDom.current.focus();
      return;
    }
    if (!typ) {
      showNotification({ severity: "warning", title: "type is empty" });
      return;
    }
    setLoading(true);
    unwrapCommand(
      commands.addTableColumn(uuid, table, name, {
        typ: typ,
        nullable: nullable ? nullable : null,
        after: after ? after : null,
        default_value: defaultValue ? defaultValue : null,
      } as ColumnType),
    )
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Add Column OK",
          message: <CopySql sql={sql} />,
        });
        handleClose();
        reload();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Add Column failure",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const supportNull = kindSupportNull(kind);
  const supportDefault = kindSupportDefault(kind);
  return (
    <Stack>
      <ClearableTextField
        value={name}
        required
        inputRef={nameDom}
        onChange={(e) => {
          setName(e.target.value.trim());
        }}
        onClear={() => setName("")}
        label="column name"
      />
      <StackRowCenter>
        <TypeSelect
          uuid={uuid}
          handleChange={(e) => {
            setTyp(e);
          }}
          style={{ flex: 1 }}
          type={typ}
        />
        {supportNull && (
          <ColumnNullness
            nullable={nullable}
            handleNullable={(e) => {
              setNullness(e);
            }}
          />
        )}
      </StackRowCenter>

      {supportDefault && (
        <SqlDefaultValue
          uuid={uuid}
          defaultValue={defaultValue}
          setDefaultValue={setDefaultValue}
        />
      )}
      {supportAfter && (
        <Autocomplete
          freeSolo={false}
          options={fields.map((e) => e.field)}
          style={{ width: "22rem" }}
          value={after}
          renderInput={(params) => <TextField {...params} label="after column" />}
          onChange={(_, field) => {
            setAfter(field);
          }}
        />
      )}
      <Divider />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleAdd}>
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function ModifyTableColumn(props: {
  uuid: string;
  table: string;
  col: string;
  handleClose;
  reload;
  typ: string;
  kind: DataSourceKind;
  defaultValue: string;
}) {
  const { uuid, table, col, handleClose, reload, typ, kind, defaultValue } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(col);
  const [newTyp, setNewTyp] = useState(typ);
  const { showNotification } = useNotification();
  const [modifyType, setModifyType] = useState(false);
  const supportModifyType = ![DataSourceKind.Cassandra].includes(kind);
  const supportModifyDefault = ![DataSourceKind.SqlServer].includes(kind);
  const [nullable, setNullness] = useState<null | SqlNullAble>(null);
  const [newDv, setNewDv] = useState(supportModifyDefault ? defaultValue : null);
  const handleModify = () => {
    if (newName == col && !newTyp) {
      showNotification({ severity: "warning", title: "nothing is changed" });
      return;
    }
    if (!newName) {
      showNotification({ severity: "error", title: "name can't empty" });
      return;
    }
    setLoading(true);
    let typePara: ColumnType = null;
    if (modifyType) {
      typePara = {
        typ: newTyp,
        nullable: nullable ? nullable : nullable,
        default_value: newDv ? newDv : null,
        after: null,
      };
    }
    unwrapCommand(commands.modifyTableColumn(uuid, table, col, newName, typePara))
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Modify column OK",
          message: <CopySql sql={sql} />,
        });
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Modify column Failure",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <ClearableTextField
        fullWidth
        label="New Name"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        required
        onClear={() => {
          setNewName("");
        }}
        error={!newName}
      />
      {supportModifyType && !modifyType && (
        <FormControlLabel
          control={
            <Checkbox
              checked={modifyType}
              onChange={(e) => {
                setModifyType(e.target.checked);
              }}
              name="myCheckbox"
              color="primary"
            />
          }
          label="modify TYPE"
        />
      )}

      {modifyType && (
        <StackRowCenter>
          <TypeSelect
            uuid={uuid}
            handleChange={(e) => {
              setNewTyp(e);
            }}
            style={{ flex: 1 }}
            type={typ}
          />
          {kindSupportNull(kind) && (
            <ColumnNullness nullable={nullable} handleNullable={setNullness} />
          )}

          {supportModifyDefault && (
            <SqlDefaultValue
              defaultValue={newDv}
              uuid={uuid}
              setDefaultValue={(v) => {
                setNewDv(v);
              }}
            />
          )}
        </StackRowCenter>
      )}
      <Stack direction="row">
        <Button loading={loading} color="error" onClick={handleModify}>
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function kindSupportNull(kind: DataSourceKind): boolean {
  return kind != DataSourceKind.Cassandra;
}
function kindSupportDefault(kind: DataSourceKind): boolean {
  return kind != DataSourceKind.Cassandra;
}

function triggerIsEnable(enable: TriggerEnable): boolean {
  if (enable.kind == "postgres") {
    return enable.enable.toUpperCase() != "D";
  }
  if (enable.kind == "bool") {
    return enable.enable;
  }
}

function TriggerMenu(props: {
  content: string;
  detail: TriggerDetail;
  handleEnables;
  handleDrop;
}) {
  const { content, detail, handleEnables, handleDrop } = props;
  const { showNotification } = useNotification();
  let enable: ReactNode;
  if (detail.enable) {
    enable = triggerIsEnable(detail.enable) ? (
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleEnables("disable_trigger", detail.name, "Disable trigger");
        }}>
        <ListItemIcon>
          <CancelIcon />
        </ListItemIcon>
        <ListItemText>Disable</ListItemText>
      </MenuItem>
    ) : (
      <MenuItem
        onClick={() => {
          handleEnables("enable_trigger", detail.name, "Enable trigger");
        }}>
        <ListItemIcon>
          <CheckCircleIcon />
        </ListItemIcon>
        <ListItemText>Enable</ListItemText>
      </MenuItem>
    );
  }

  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      {enable}
      <MenuItem
        onClick={() => {
          handleDrop(detail.name);
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> drop </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function TriggersStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useUserSettings();
  const [loading, setLoading] = useState(false);
  const [triggers, setTriggers] = useState<TriggerDetail[]>([]);
  const [row, setRow] = useState<TriggerDetail>(null);
  const { showNotification } = useNotification();

  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

  const handleReload = useCallback(() => {
    setLoading(true);

    commands
      .showTrigger(uuid, table)
      .then((ret) => {
        if (ret.status == "ok") {
          setTriggers(ret.data);
        } else {
          showNotification({
            severity: "error",
            title: "list indexes failed",
            message: ret.error,
          });
        }
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);
  const callPara = useMemo(() => {
    return { name: uuid, tableName: table };
  }, [table, uuid]);

  const handleDrop = useCallback(
    async (trigger) => {
      const confirm = await secondConfirm({
        title: `Drop trigger '${trigger}'`,
        confirmText: "Drop",
        kind: "error",
      });
      if (!confirm) {
        return;
      }
      unwrapCommand(commands.dropTrigger(undefined, undefined, trigger))
        .then((sql) => {
          showNotification({
            severity: "success",
            title: "Drop Trigger OK",
            message: <CopySql sql={sql} />,
          });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: "Drop trigger failure",
            message: e,
          });
        });
    },
    [handleReload, showNotification],
  );

  const handleEnables = useCallback(
    (invoke_name: string, trigger, operation: string) => {
      invoke(invoke_name, { trigger, ...callPara })
        .then((sql) => {
          showNotification({
            severity: "success",
            title: `${operation}  OK`,
            message: <CopySql sql={sql} />,
          });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: `${operation}  Failure`,
            message: e,
          });
        });
    },
    [callPara, handleReload, showNotification],
  );

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [
      { field: "name" },
      { field: "event" },
      { field: "sql" },
      {
        field: "enable",
        renderCell: (r) => {
          const enable = r.row.enable as TriggerEnable;
          if (!enable) {
            return;
          }
          switch (enable.kind) {
            case "postgres":
              return enable.enable;
            case "bool":
              return <TrueOrFalse value={enable.enable} />;
          }
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          {
            const { field } = extractFieldAndIDFromMouseEvent(e);
            if (!field) {
              return;
            }
          }
          const content = (target as HTMLDivElement).innerHTML;
          const name = (currentTarget as HTMLDivElement).getAttribute("data-id");
          const detail = triggers.find((e) => e.name == name);
          setUpMenu({
            content: (
              <TriggerMenu
                detail={detail}
                content={content}
                handleDrop={handleDrop}
                handleEnables={handleEnables}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export_.csvDelimiter,
          includeHeaders: setting.export_.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    handleEnables,
    setUpMenu,
    setting.export_.csvDelimiter,
    setting.export_.includeHeader,
    triggers,
  ]);

  const getRowId = useCallback((e) => {
    return e["name"];
  }, []);
  const onRowDoubleClick = useCallback((row) => {
    setRow(row.row);
  }, []);

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);

  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" style={{ flex: 1, justifyContent: "space-between" }}>
            <Stack direction="row">
              {triggers.length > 0 && (
                <Tooltip title="double click a row to see detail">
                  <Typography color="info">detail</Typography>
                </Tooltip>
              )}
            </Stack>
            <AppDataGridPagination />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [triggers.length]);
  return (
    <Stack style={{ height: "100%" }}>
      <StackRowCenter>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>
      </StackRowCenter>

      <DataGrid
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        columns={cols}
        rows={triggers}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
        slots={slots}
        loading={loading}
      />

      <ListenElementSize dom={modalBox} set="both" name="trigger-detail" />

      {row && (
        <Modal
          open={!!row}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRow(null);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80vw", height: "80vh" }}
            ref={setModalBox}>
            <ModalHeader
              title={`Trigger Detail`}
              handleClose={() => {
                setRow(null);
              }}
            />
            <TriggerDetailView
              reloadOP={async () => {
                return row;
              }}
              uuid={uuid}
              kind={kind}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function TableStructure(props: { table: string; uuid: string; kind: DataSourceKind }) {
  const { table, uuid, kind } = props;
  const [structure, setStructure] = useState("column");
  const structures = ["column"];

  const supportStatement = [
    DataSourceKind.Cassandra,
    DataSourceKind.ClickHouse,
    DataSourceKind.LibSql,
    DataSourceKind.Sqlite,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.MariaDB,
    DataSourceKind.DuckDB,
  ].includes(kind);
  if (supportStatement) {
    structures.push("statement");
  }
  const supportIndex = [
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.SqlServer,
    DataSourceKind.LibSql,
    DataSourceKind.Sqlite,
    DataSourceKind.Oracle,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.LibSql,
    DataSourceKind.Mongo,
    DataSourceKind.Cassandra,
    DataSourceKind.FireBird,
    DataSourceKind.DuckDB,
  ].includes(kind);

  if (supportIndex) {
    structures.push("index");
  }
  const supportTrigger = [
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Oracle,
    DataSourceKind.SqlServer,
    DataSourceKind.Postgres,
  ].includes(kind);
  if (supportTrigger) {
    structures.push("trigger");
  }

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" sx={{ justifyContent: "center" }}>
        <ButtonGroup>
          {structures.map((e) => {
            return (
              <Button
                disabled={structure == e}
                onClick={() => {
                  setStructure(e);
                }}
                key={e}>
                {e}
              </Button>
            );
          })}
        </ButtonGroup>
      </Stack>

      {structure == "column" && (
        <ColumnsStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "index" && (
        <IndexesStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "trigger" && (
        <TriggersStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "statement" && (
        <StatementStructure table={table} kind={kind} uuid={uuid} />
      )}
    </Stack>
  );
}
function StatementStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const { showNotification } = useNotification();

  return (
    <Stack style={{ height: "100%" }}>
      <DefAndMetasView
        reloadOP={async () => {
          return commands.showCreateTableSql(uuid, table).then((ret) => {
            if (ret.status == "ok") {
              return {
                def: formatCreateTableAndIndexSql(ret.data),
                metas: [],
              } as DefAndMetas;
            } else {
              showNotification({
                severity: "error",
                title: "show create table sql failed",
                message: ret.error,
              });
              return {
                def: "",
                metas: [],
              } as DefAndMetas;
            }
          });
        }}
        kind={kind}
        uuid={uuid}
      />
    </Stack>
  );
}

export { TableStructure };
