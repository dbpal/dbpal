import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { EscDownEvent, modalHandleClose } from "./Common";
import {
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  secondConfirm,
  DbnameContext,
} from "./helper";
import { AppDataGridToolBar, ClearableTextField, RefreshButton } from "./CommonComp2";
import CancelIcon from "@mui/icons-material/Cancel";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import {
  Autocomplete,
  Button,
  ButtonGroup,
  Checkbox,
  Dialog,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
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
import { ModalHeader, CheckedView } from "./CommonComp";
import { DefAndMetasView, TriggerDetailView } from "./DefAndMetaView";
import { formatCreateTableAndIndexSql } from "./handlers";
import {
  AppDataGridPagination,
  ColumnNullness,
  CopySql,
  CreateIndexView,
  SqlDefaultValue,
  StackRowCenter,
  TypeSelect,
} from "./CommonComp2";
import { useColumnWidthStore, useContextMenu, useNotification } from "./store";
import {
  ColumnType,
  commands,
  CreateIndexReq,
  Field,
  ForeignKeyDef,
  Index,
  SqlNullAble,
  TriggerDetail,
  TriggerEnable,
} from "./bindings";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "./datagridExport";

/**
 * Context menu for a column row, supporting copy and optional drop actions.
 */
function ColumnMenu(props: {
  col: string;
  content: string;
  supportModifyColumn: boolean;
  handleDrop: (col: string) => void;
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

function ColumnsStructure(props: { table: string }) {
  const { dbName, kind } = useContext(DbnameContext);
  const { table } = props;
  const [loading, setLoading] = useState(false);
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [row, setRow] = useState<Field>(null);
  const { getColumnWidth } = useColumnWidthStore();
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

  const handleReload = useCallback(() => {
    setLoading(true);

    commands
      .showTableColumns(dbName, table)
      .then((data) => {
        data.forEach((e, index) => {
          e["id"] = index;
        });
        setFields(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list column failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, dbName]);

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
      commands
        .dropTableColumn(dbName, table, col)
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
    [handleReload, showNotification, table, dbName],
  );
  const [showAdd, setShowAdd] = useState(false);
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols: GridColDef[] = useMemo(() => {
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
          return <CheckedView value={row.row["nullable"]} />;
        },
      },
      {
        field: "is_primary",
        renderHeader: () => {
          return "primary key";
        },
        renderCell: (row) => {
          return <CheckedView value={row.row["is_primary"]} />;
        },
      },
      {
        field: "is_auto_increment",
        renderHeader: () => {
          return "auto increment";
        },
        renderCell: (row) => {
          return <CheckedView value={row.row["is_auto_increment"]} />;
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
          return <CheckedView value={row.row["sortable"]} />;
        },
      },
    ];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo((): GridSlotsComponentsProps => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = fields.find((e) => e["id"] == id);

          const content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: (
              <ColumnMenu
                supportModifyColumn={supportModifyColumn}
                col={row.field}
                content={content}
                handleDrop={handleDrop}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    };
  }, [fields, handleDrop, setUpMenu, supportModifyColumn]);

  const onRowDoubleClick = useCallback((row: { row: Field }) => {
    setRow(row.row);
  }, []);
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
  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName={table}
            left=<StackRowCenter>
              <RefreshButton loading={loading} onClick={handleReload} />
              {supportModifyColumn && (
                <IconButton
                  onClick={() => {
                    setShowAdd(true);
                  }}>
                  <AddIcon />
                </IconButton>
              )}
            </StackRowCenter>
          />
        );
      },
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
    };
  }, [handleReload, loading, supportModifyColumn, table]);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        slots={slots}
        slotProps={slotProps}
        columns={cols}
        rows={fields}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
        loading={loading}
      />

      {showAdd && (
        <Dialog
          fullWidth
          maxWidth="md"
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <DialogTitle> add table column </DialogTitle>
          <TableAddColumn
            table={table}
            dbName={dbName}
            handleClose={() => {
              setShowAdd(false);
            }}
            reload={handleReload}
            kind={kind}
            fields={fields}
          />
        </Dialog>
      )}
      {supportModifyColumn &&
        row &&
        ![DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind) && (
          <Dialog
            open={!!row}
            fullWidth
            maxWidth="md"
            onClose={modalHandleClose((e: EscDownEvent) => {
              e.preventDefault();
              setRow(null);
            })}>
            <DialogTitle> modify table column </DialogTitle>
            <ModifyTableColumn
              table={table}
              dbName={dbName}
              handleClose={() => {
                setRow(null);
              }}
              col={row.field}
              typ={row.data_type}
              reload={handleReload}
              kind={kind}
              defaultValue={row.default_value}
            />
          </Dialog>
        )}
    </Stack>
  );
}

function RenameIndex(props: {
  dbName: string;
  table: string;
  reload: () => void;
  handleClose: () => void;
  indexName: string;
}) {
  const { dbName, table, reload, handleClose, indexName } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewname] = useState(indexName);
  const { showNotification } = useNotification();
  const handleRename = () => {
    if (newName == indexName) {
      showNotification({ severity: "warning", title: "name not change" });
      return;
    }
    setLoading(true);

    commands
      .renameIndex(dbName, table, indexName, newName)
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
  setRenameIndexName: (indexName: string) => void;
  handleDrop: (indexName: string) => void;
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

function IndexesStructure(props: { table: string }) {
  const { dbName, kind } = useContext(DbnameContext);
  const { table } = props;
  const [loading, setLoading] = useState(false);
  const [indexes, setIndexes] = useState<Index[]>([]);
  const { showNotification } = useNotification();
  const [renameIndexName, setRenameIndexName] = useState("");
  const [showAdd, setShowAdd] = useState(false);

  const { getColumnWidth } = useColumnWidthStore();

  const handleReload = useCallback(() => {
    setLoading(true);

    commands
      .showIndexes(dbName, table)
      .then((data) => {
        data.forEach((x, index) => {
          x["id"] = index;
        });
        setIndexes(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "show indexes failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, dbName]);

  const handleDrop = useCallback(
    async (indexName: string) => {
      const confirm = await secondConfirm({
        title: `drop index '${indexName}'`,
        content: "Are u sure to drop the index",
        confirmText: "Drop",
      });
      if (!confirm) {
        return;
      }
      commands
        .dropIndex(dbName, table, indexName)
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
    [handleReload, showNotification, table, dbName],
  );
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo((): GridColDef[] => {
    return [{ field: "name" }, { field: "kind" }, { field: "columns" }];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo((): GridSlotsComponentsProps => {
    return {
      row: {
        onContextMenu: (e) => {
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          let row = indexes.find((e) => e["id"] == id);

          setUpMenu({
            content: (
              <IndexMenu
                indexName={row.name}
                setRenameIndexName={setRenameIndexName}
                handleDrop={handleDrop}
                content={(e.target as HTMLDivElement).innerHTML}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    };
  }, [handleDrop, indexes, setUpMenu]);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName={table}
            left=<Stack direction="row">
              <RefreshButton loading={loading} onClick={handleReload} />
              <IconButton
                onClick={() => {
                  setShowAdd(true);
                }}>
                <AddIcon />
              </IconButton>
            </Stack>
          />
        );
      },
      pagination: AppDataGridPagination,
    };
  }, [handleReload, loading, table]);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        slotProps={slotProps}
        columns={cols}
        rows={indexes}
        style={dataGridStyle}
        slots={slots}
        loading={loading}
      />

      {showAdd && kind == DataSourceKind.Mongo && (
        <Dialog
          open={showAdd}
          fullWidth
          maxWidth="md"
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}>
          <DialogTitle> mongo create index </DialogTitle>
          <div style={{ height: "40vh" }}>
            <MongoAddIndex
              dbName={dbName}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
            />
          </div>
        </Dialog>
      )}
      {showAdd && kind != DataSourceKind.Mongo && (
        <Dialog
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
          fullWidth
          maxWidth="md">
          <DialogTitle> create index </DialogTitle>
          <AddIndex
            dbName={dbName}
            table={table}
            handleClose={() => {
              setShowAdd(false);
            }}
            kind={kind}
            reload={handleReload}
          />
        </Dialog>
      )}

      {renameIndexName && (
        <Dialog
          open={!!renameIndexName}
          onClose={(e: EscDownEvent) => {
            e.preventDefault();
            setRenameIndexName("");
          }}>
          <DialogTitle> rename index </DialogTitle>
          <RenameIndex
            dbName={dbName}
            table={table}
            indexName={renameIndexName}
            handleClose={() => {
              setRenameIndexName("");
            }}
            reload={handleReload}
          />
        </Dialog>
      )}
    </Stack>
  );
}

function MongoAddIndex(props: {
  dbName: string;
  table: string;
  handleClose: () => void;
  reload: () => void;
}) {
  const { dbName, table, handleClose, reload } = props;

  const indexTypes = ["normal", "unique"];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState("");
  const indexNameDom = useRef<HTMLInputElement>(null);
  const { showNotification } = useNotification();

  const handleAdd = async (keys: Record<string, any>) => {
    try {
      if (!indexName) {
        showNotification({ severity: "warning", title: "index name can't be none" });
        indexNameDom.current?.focus();
        return;
      }
      await commands.mongoCreateIndex(
        dbName,
        table,
        indexName ? indexName : null,
        BSON.serialize(keys),
        indexType == "unique",
      );
      showNotification({ severity: "success", title: "create index ok" });
      handleClose();
      reload();
    } catch (e) {
      showNotification({ severity: "error", title: "create index failed", message: e });
    }
  };

  return (
    <Stack style={{ flex: 1, height: "100%" }}>
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
        needConfirm={false}
      />
    </Stack>
  );
}

function AddIndex(props: {
  dbName: string;
  table: string;
  handleClose: () => void;
  reload: () => void;
  kind: DataSourceKind;
}) {
  const { dbName, table, handleClose, reload, kind } = props;
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [req, setReq] = useState<CreateIndexReq>(null);
  useEffect(() => {
    commands
      .showTableColumns(dbName, table)
      .then((data) => {
        setFields(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list fields failed",
          message: e,
        });
      });
  }, [showNotification, table, dbName]);
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
      const sql = await commands.createIndex(dbName, table, req);
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
    <Stack style={{ height: "100%" }}>
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
  dbName: string;
  handleClose: () => void;
  reload: () => void;
  kind: DataSourceKind;
  fields: Field[];
}) {
  const { table, dbName, handleClose, reload, kind, fields } = props;
  const [name, setName] = useState("");
  const [nullable, setNullness] = useState<SqlNullAble>("unspecified");
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

    commands
      .addTableColumn(dbName, table, name, {
        typ: typ,
        nullable: nullable ? nullable : null,
        after: after ? after : null,
        default_value: defaultValue ? defaultValue : null,
      })

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
  dbName: string;
  table: string;
  col: string;
  handleClose: () => void;
  reload: () => void;
  typ: string;
  kind: DataSourceKind;
  defaultValue: string;
}) {
  const { dbName, table, col, handleClose, reload, typ, kind, defaultValue } = props;
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
    commands
      .modifyTableColumn(dbName, table, col, newName, typePara)
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

// database kind support null specific
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
  handleEnables: (triggerName: string, enable: boolean) => void;
  handleDrop: (trigger: string) => void;
}) {
  const { content, detail, handleEnables, handleDrop } = props;
  const { showNotification } = useNotification();
  let enable: ReactNode;
  if (detail.enable) {
    enable = triggerIsEnable(detail.enable) ? (
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleEnables(detail.name, false);
        }}>
        <ListItemIcon>
          <CancelIcon />
        </ListItemIcon>
        <ListItemText>Disable</ListItemText>
      </MenuItem>
    ) : (
      <MenuItem
        onClick={() => {
          handleEnables(detail.name, true);
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

function TriggersStructure(props: { table: string }) {
  const { table } = props;
  const { dbName } = useContext(DbnameContext);
  const [loading, setLoading] = useState(false);
  const [triggers, setTriggers] = useState<TriggerDetail[]>([]);
  const [row, setRow] = useState<TriggerDetail>(null);
  const { showNotification } = useNotification();

  const { getColumnWidth } = useColumnWidthStore();

  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .showTrigger(dbName, table)
      .then((data) => {
        data.forEach((e, index) => {
          e["id"] = index;
        });
        setTriggers(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list indexes failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, dbName]);
  const handleDrop = useCallback(
    async (trigger: string) => {
      const confirm = await secondConfirm({
        title: `Drop trigger '${trigger}'`,
        confirmText: "Drop",
        kind: "error",
      });
      if (!confirm) {
        return;
      }
      commands
        .dropTrigger(undefined, undefined, trigger)
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
    (triggerName: string, enable: boolean) => {
      let title = enable ? "Enable" : "Disable";
      commands
        .enableTrigger(dbName, table, triggerName, enable)
        .then((sql) => {
          showNotification({
            severity: "success",
            title: `${title} OK`,
            message: <CopySql sql={sql} />,
          });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            title: `${title} Failure`,
            message: e,
          });
        });
    },
    [handleReload, showNotification, table, dbName],
  );

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo((): GridColDef[] => {
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
              return <CheckedView value={enable.enable} />;
          }
        },
      },
    ];
  }, []);
  cols.forEach((e) => {
    const width = getColumnWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const slotProps = useMemo((): GridSlotsComponentsProps => {
    return {
      row: {
        onContextMenu: (e) => {
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const detail = triggers.find((e) => e["id"] == id);
          setUpMenu({
            content: (
              <TriggerMenu
                detail={detail}
                content={(e.target as HTMLDivElement).innerHTML}
                handleDrop={handleDrop}
                handleEnables={handleEnables}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    };
  }, [handleDrop, handleEnables, setUpMenu, triggers]);

  const onRowDoubleClick = useCallback((row: { row: TriggerDetail }) => {
    setRow(row.row);
  }, []);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName={table}
            left=<StackRowCenter>
              <RefreshButton loading={loading} onClick={handleReload} />
            </StackRowCenter>
          />
        );
      },
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
    };
  }, [handleReload, loading, table, triggers.length]);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        slotProps={slotProps}
        columns={cols}
        rows={triggers}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
        slots={slots}
        loading={loading}
      />
      {row && (
        <Dialog
          open={!!row}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRow(null);
          })}
          fullScreen
          maxWidth="lg"
          disableAutoFocus
          disableEnforceFocus>
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
          />
        </Dialog>
      )}
    </Stack>
  );
}

function TableStructure(props: { table: string }) {
  const { kind } = useContext(DbnameContext);
  const { table } = props;
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

  const supportForeignKey = [
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Oracle,
    DataSourceKind.FireBird,
    DataSourceKind.DuckDB,
    DataSourceKind.SqlServer,
  ].includes(kind);

  if (supportForeignKey) {
    structures.push("foreign_key");
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
      {structure == "column" && <ColumnsStructure table={table} />}
      {structure == "index" && <IndexesStructure table={table} />}
      {structure == "trigger" && <TriggersStructure table={table} />}
      {structure == "statement" && <StatementStructure table={table} />}{" "}
      {structure == "foreign_key" && <ForeignKeyStructure table={table} />}
    </Stack>
  );
}

function ForeignKeyStructure(props: { table: string }) {
  const { table } = props;
  const { dbName } = useContext(DbnameContext);
  const [loading, setLoading] = useState(false);
  const [fs, setFs] = useState<ForeignKeyDef[]>([]);
  const { showNotification } = useNotification();
  const { setUpMenu } = useContextMenu();

  const { getColumnWidth } = useColumnWidthStore();
  const handleReload = useCallback(() => {
    setLoading(false);
    commands
      .listForeignKey(dbName, table)
      .then((fs) => {
        fs.forEach((e, index) => {
          e["id"] = index;
        });
        setFs(fs);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list foreign key failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [dbName, showNotification, table]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);

  const columns: GridColDef[] = [
    { field: "name" },
    { field: "table" },
    { field: "from" },
    { field: "to" },
    { field: "on_update" },
    { field: "on_delete" },
  ];

  columns.forEach((e) => {
    e.width = getColumnWidth(e.field);
  });

  const handleDrop = useCallback(
    (name) => {
      commands
        .dropForeignKey(dbName, table, name)
        .then(() => {
          showNotification({ severity: "success", title: "drop foreign key success" });
          handleReload();
        })
        .catch((e) => {
          showNotification({
            severity: "error",
            message: e,
            title: "drop foreign key failed",
          });
        });
    },
    [dbName, handleReload, showNotification, table],
  );

  const slotProps = useMemo((): GridSlotsComponentsProps => {
    return {
      row: {
        onContextMenu: (e) => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = fs.find((e) => e["id"] == id);
          if (!row.name) {
            return;
          }
          setUpMenu({
            content: (
              <MenuItem
                sx={{ color: "error.main" }}
                onClick={() => {
                  handleDrop(row.name);
                }}>
                <ListItemIcon>
                  <DeleteForeverIcon color="error" />
                </ListItemIcon>
                <ListItemText> Drop</ListItemText>
              </MenuItem>
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    };
  }, [fs, handleDrop, setUpMenu]);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName={table}
            left=<StackRowCenter>
              <RefreshButton loading={loading} onClick={handleReload} />
            </StackRowCenter>
          />
        );
      },
      pagination: AppDataGridPagination,
    };
  }, [handleReload, loading, table]);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        style={dataGridStyle}
        columns={columns}
        slots={slots}
        slotProps={slotProps}
        rows={fs}
      />
    </Stack>
  );
}
function StatementStructure(props: { table: string }) {
  const { dbName } = useContext(DbnameContext);
  const { table } = props;
  const { showNotification } = useNotification();

  return (
    <Stack style={{ height: "100%" }}>
      <DefAndMetasView
        reloadOP={async () => {
          return commands
            .showCreateTableSql(dbName, table)
            .then((data) => {
              return {
                def: formatCreateTableAndIndexSql(data),
                metas: [],
              };
            })
            .catch((e) => {
              showNotification({
                severity: "error",
                title: "show create table sql failed",
                message: e,
              });
              return {
                def: "",
                metas: [],
              };
            });
        }}
      />
    </Stack>
  );
}

export { TableStructure };
