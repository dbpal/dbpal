import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { modalStyle, EscDownEvent, modalHandleClose, uuidv4 } from "./Common.ts";

import {
  Box,
  Button,
  Checkbox,
  CircularProgress,
  Dialog,
  Divider,
  FormControlLabel,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
  TextField,
} from "@mui/material";

import { SqlTableView, SqlTableViewData } from "./SqlTableView";
import { ExportTableOrView } from "./Export";
import AddIcon from "@mui/icons-material/Add";
import {
  RedisAddHash,
  RedisAddList,
  RedisAddSet,
  RedisAddZset,
  RedisHashTable,
  RedisListTable,
  RedisSetTable,
  RedisStringTable,
  RedisZsetTable,
} from "./Redis";
import { MongoTable, MongoTableAdd } from "./MongoDB";
import { BSON } from "bson";
import {
  DataSourceKind,
  secondConfirm,
  DbnameContext,
  handleCopyToClipboard,
  RedisTableType,
  TabContent,
  textEllipsisStyle,
} from "./helper";
import {
  MenuRefresh,
  ModalHeader,
  MyTreeItem,
  LeftTableNameAndColumns,
  VirtualListLeft,
} from "./CommonComp";
import { TableStructure } from "./TableStructure";
import { cancelTask } from "./handlers";
import {
  CancelButton,
  ClearableTextField,
  CopySql,
  ListenElementSize,
  ShowingCount,
} from "./CommonComp2";
import { CouchBaseInsert, CouchBaseTable } from "./CouchBase";
import {
  useNotification,
  useTabsManage,
  useMatcherFn,
  useOneShotDialogUI,
} from "./store.tsx";
import { commands, Field, ListTable, RedisListTableRet } from "./bindings.ts";
import {
  CouchbaseTableImportFile,
  MongoTableImportFile,
  SqlCreateTable,
  SqlCreateTableImportFile,
} from "./CreateTable.tsx";
import { useDebounce } from "use-debounce";
import {
  ArrowCircleDownIcon,
  ContentCopyIcon,
  CreateIcon,
  DeleteForeverIcon,
  EditIcon,
  HiOutlineDuplicate,
  IoConstructOutline,
  ListIcon,
  PlaylistRemoveIcon,
  TableViewIcon,
  UploadIcon,
} from "./icons.tsx";

function TablesTree() {
  const { dbName, kind } = useContext(DbnameContext);
  const [tables, setTables] = useState<ListTable[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);
  const { appendTab } = useTabsManage();

  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return tables.filter((e) => matchPattern(e.name));
  }, [tables, matchPattern]);

  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .listTable(dbName)
      .then((data) => {
        setTables(data);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list table failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, dbName]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  let tablesText: string;

  switch (kind) {
    case DataSourceKind.CouchBase:
    case DataSourceKind.Mongo:
      tablesText = "collections";
      break;
    default: {
      tablesText = "tables";
    }
  }
  const listTableCols = useCallback(
    (name: ListTable) => {
      return commands.showTableColumns(dbName, name.name);
    },
    [dbName],
  );
  return (
    <MyTreeItem
      enable={true}
      makeMenu={() => {
        return (
          <MenuList>
            <MenuRefresh refresh={handleReload} />
            {kind != DataSourceKind.Redis && (
              <>
                {kind != DataSourceKind.Mongo && kind != DataSourceKind.CouchBase && (
                  <MenuItem
                    onClick={() => {
                      const content = <SqlCreateTable />;
                      appendTab(
                        new TabContent(
                          `create table`,
                          content,
                          <CreateIcon />,
                          dbName,
                          kind,
                        ),
                      );
                    }}>
                    <ListItemIcon>
                      <CreateIcon />
                    </ListItemIcon>
                    <ListItemText>create table </ListItemText>
                  </MenuItem>
                )}

                <MenuItem
                  onClick={() => {
                    let content = <SqlCreateTableImportFile />;
                    if (kind == DataSourceKind.Mongo) {
                      content = <MongoTableImportFile />;
                    }
                    if (kind == DataSourceKind.CouchBase) {
                      content = <CouchbaseTableImportFile />;
                    }
                    appendTab(
                      new TabContent(
                        `create table`,
                        content,
                        <UploadIcon />,
                        dbName,
                        kind,
                      ),
                    );
                  }}>
                  <ListItemIcon>
                    <UploadIcon />
                  </ListItemIcon>
                  <ListItemText>create table import file</ListItemText>
                </MenuItem>
              </>
            )}
          </MenuList>
        );
      }}
      label=<>
        <TableViewIcon />
        <span> {tablesText} </span>
        <ShowingCount showing={showItems.length} total={tables.length} />
        {loading && <CircularProgress />}
      </>>
      {showItems.map((table) => (
        <LeftTableNameAndColumns
          key={table.name}
          makeMenu={() => {
            return (
              <TableMenu
                kind={kind}
                reload={handleReload}
                table={table.name}
                dbName={dbName}
              />
            );
          }}
          table={table}
          listCols={listTableCols}
        />
      ))}
    </MyTreeItem>
  );
}

function DropTable(props: {
  table: string;
  reloadTables: () => void;
  dbName: string;
  handleClose: () => void;
  kind: DataSourceKind;
}) {
  const { table, reloadTables, dbName, handleClose, kind } = props;
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const [cascade, setCascade] = useState(false);

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      confirmText: "Drop",
      title: `drop table '${table}'`,
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    setLoading(true);
    commands
      .dropTable(dbName, table, cascade)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop table OK",
          message: <CopySql sql={sql} />,
        });
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "Drop table failure",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const supportCascade = (): boolean => {
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
      case DataSourceKind.Oracle:
        return true;
      default:
        return false;
    }
  };

  return (
    <Stack>
      {supportCascade() && (
        <FormControlLabel
          control={
            <Checkbox
              checked={cascade}
              onChange={(e) => setCascade(e.target.checked)}
            />
          }
          label="CASCADE"
        />
      )}
      <Stack direction="row">
        <Button loading={loading} color="error" onClick={handleDrop}>
          drop
        </Button>
      </Stack>
    </Stack>
  );
}
interface RedisViewTableProps {
  dbName: string;
  table: string;
  redisType: RedisTableType;
}

// 1. Rename to start with an Uppercase letter
function RedisViewTable({ table, redisType }: RedisViewTableProps) {
  // 2. Using a mapping object is cleaner than a switch statement
  switch (redisType) {
    case RedisTableType.String:
      return <RedisStringTable stringKey={table} />;
    case RedisTableType.List:
      return <RedisListTable listKey={table} />;
    case RedisTableType.Set:
      return <RedisSetTable setKey={table} />;
    case RedisTableType.Zset:
      return <RedisZsetTable zsetKey={table} />;
    case RedisTableType.Hash:
      return <RedisHashTable hashKey={table} />;
    default:
      return null; // Fallback for safety
  }
}

function makeViewTable(
  dbName,
  table: string,
  kind: DataSourceKind,
  redisTableType: RedisTableType,
) {
  let tableList: ReactNode = null;
  switch (kind) {
    case DataSourceKind.MariaDB:
    case DataSourceKind.Mysql:
    case DataSourceKind.SqlServer:
    case DataSourceKind.Sqlite:
    case DataSourceKind.Postgres:
    case DataSourceKind.Oracle:
    case DataSourceKind.TiDB:
    case DataSourceKind.Cassandra:
    case DataSourceKind.ClickHouse:
    case DataSourceKind.LibSql:
    case DataSourceKind.FireBird:
    case DataSourceKind.DuckDB:
    case DataSourceKind.Cockroach:
      tableList = <SqlTableView isTable tableName={table} />;
      break;
    case DataSourceKind.Redis:
      tableList = (
        <RedisViewTable dbName={dbName} table={table} redisType={redisTableType} />
      );
      break;
    case DataSourceKind.Mongo:
      tableList = <MongoTable table={table} isTable />;
      break;
    case DataSourceKind.CouchBase:
      tableList = <CouchBaseTable table={table} />;
  }
  return tableList;
}

function AddTableView(props: {
  dbName: string;
  table: string;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  const { dbName, table, kind, redisTableType } = props;

  switch (kind) {
    case DataSourceKind.Redis:
      switch (redisTableType) {
        case RedisTableType.String:
          return <RedisStringTable stringKey={table} />;
        case RedisTableType.List:
          return <RedisAddList listKey={table} />;
        case RedisTableType.Set:
          return <RedisAddSet setKey={table} />;
        case RedisTableType.Zset:
          return <RedisAddZset zsetKey={table} />;
        case RedisTableType.Hash:
          return <RedisAddHash hashKey={table} />;
      }
      break;

    case DataSourceKind.Mongo:
      return (
        <MongoTableAdd
          addOP={async (data) => {
            return commands.mongoInsert(dbName, table, BSON.serialize(data));
          }}
        />
      );
    case DataSourceKind.CouchBase:
      return <CouchBaseInsert dbName={dbName} table={table} />;
  }

  return <SqlAddTableView dbName={dbName} table={table} />;
}

function AddTableModal(props: {
  dbName: string;
  tableName: string;
  kind: DataSourceKind;
  redisType?: RedisTableType;
}) {
  const { tableName, redisType, dbName, kind } = props;
  const [showAdd, setShowAdd] = useState(false);
  const [addModalBox, setAddModalBox] = useState<HTMLDivElement | null>(null);

  return (
    <>
      <IconButton
        onClick={() => {
          setShowAdd(true);
        }}
        color="primary">
        <AddIcon />
      </IconButton>

      <ListenElementSize
        dom={addModalBox}
        set="both"
        name={kind + "-add-" + redisType}
      />

      {showAdd && (
        <Dialog
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
          disableAutoFocus
          disableEnforceFocus>
          <Box
            sx={{ resize: "both", ...modalStyle, width: "80%", height: "70vh" }}
            ref={setAddModalBox}>
            <ModalHeader
              noTextTransform
              title={`add ${tableName} `}
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <AddTableView
              dbName={dbName}
              table={tableName}
              redisTableType={redisType}
              kind={kind}
            />
          </Box>
        </Dialog>
      )}
    </>
  );
}

function SqlAddTableView(props: { dbName: string; table: string }) {
  const { dbName, table } = props;
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
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
  return (
    <SqlTableViewData
      isAdd
      buttonName="add"
      handleData={async (values) => {
        await commands.tableInsert(dbName, table, values);
      }}
      fields={fields}
    />
  );
}

export const openNewTableStructure = (appendTab, dbName, kind, table) => {
  appendTab(
    new TabContent(
      `${table}`,
      <TableStructure table={table} />,
      <IoConstructOutline />,
      dbName,
      kind,
    ),
  );
};

function TableMenu(props: {
  table: string;
  dbName: string;
  reload;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  const { table, dbName, reload, kind, redisTableType } = props;
  const { showNotification } = useNotification();
  const { appendTab } = useTabsManage();
  const { setOneShotUI } = useOneShotDialogUI();

  const handleTruncateTable = async () => {
    const confirm = await secondConfirm({
      title: `Truncate '${table}'`,
      confirmText: "Truncate",
      content: "",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    commands
      .truncateTable(dbName, table)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: `Truncate table OK`,
          message: <CopySql sql={sql} />,
        });
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: `Truncate  table failure`,
          message: e,
        });
      });
  };

  const handleViewData = () => {
    const tableList: ReactNode = makeViewTable(dbName, table, kind, redisTableType);
    appendTab(new TabContent(table, tableList, <TableViewIcon />, dbName, kind));
  };
  const handleAddData = async () => {
    appendTab(
      new TabContent(
        `${table}*`,
        <AddTableView
          dbName={dbName}
          kind={kind}
          table={table}
          redisTableType={redisTableType}
        />,
        <TableViewIcon />,
        dbName,
        kind,
      ),
    );
  };

  const handleStructure = () => {
    openNewTableStructure(appendTab, dbName, kind, table);
  };

  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          handleViewData();
        }}>
        <ListItemIcon>
          <ListIcon />
        </ListItemIcon>
        <ListItemText> View Data</ListItemText>
      </MenuItem>

      {redisTableType != RedisTableType.String && (
        <MenuItem
          onClick={() => {
            handleAddData();
          }}>
          <ListItemIcon>
            <AddIcon />
          </ListItemIcon>
          <ListItemText> Add Data </ListItemText>
        </MenuItem>
      )}
      <MenuItem onClick={handleStructure}>
        <ListItemIcon>
          <IoConstructOutline style={{ fontSize: "1.2rem" }} />
        </ListItemIcon>
        <ListItemText> Structure </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={() => {
          handleCopyToClipboard(table, showNotification);
        }}>
        <ListItemIcon>
          <ContentCopyIcon />
        </ListItemIcon>
        <ListItemText> Copy Name </ListItemText>
      </MenuItem>
      <Divider />

      <MenuItem
        onClick={() => {
          setOneShotUI({
            ui: (
              <DupTable
                table={table}
                dbName={dbName}
                handleClose={() => {
                  setOneShotUI({ ui: null });
                }}
                reloadTables={reload}
              />
            ),
            title: "Duplicate table",
          });
        }}>
        <ListItemIcon>
          <HiOutlineDuplicate style={{ fontSize: "1.7rem" }} />
        </ListItemIcon>
        <ListItemText> Duplicate </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setOneShotUI({
            ui: <ExportTableOrView dbName={dbName} name={table} kind={kind} isTable />,
            title: `export table ${table}`,
          });
        }}>
        <ListItemIcon>
          <ArrowCircleDownIcon />
        </ListItemIcon>
        <ListItemText> Export </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setOneShotUI({
            ui: (
              <RenameTable
                table={table}
                dbName={dbName}
                handleClose={() => {
                  setOneShotUI({ ui: null });
                }}
                reloadTables={reload}
              />
            ),
            title: `rename table ${table}`,
          });
        }}>
        <ListItemIcon>
          <EditIcon />
        </ListItemIcon>
        <ListItemText> Rename </ListItemText>
      </MenuItem>
      <Divider />
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleTruncateTable();
        }}>
        <ListItemIcon>
          <PlaylistRemoveIcon color="error" />
        </ListItemIcon>
        <ListItemText> Truncate </ListItemText>
      </MenuItem>
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          setOneShotUI({
            ui: (
              <DropTable
                table={table}
                dbName={dbName}
                handleClose={() => {
                  setOneShotUI({ ui: null });
                }}
                reloadTables={reload}
                kind={kind}
              />
            ),
            title: `drop table ${table}`,
          });
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText>Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function DupTable(props: {
  table: string;
  handleClose;
  dbName: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, dbName, reloadTables } = props;
  const [loading, setLoading] = useState(false);
  const [name, setName] = useState(table);
  const { showNotification } = useNotification();
  const taskId = useRef(uuidv4());
  const handleRename = () => {
    if (name == table) {
      showNotification({ severity: "error", title: "name not changed" });
      return;
    }
    setLoading(true);
    commands
      .duplicateTable(dbName, table, taskId.current, name)
      .then(() => {
        showNotification({ severity: "success", title: "duplicate OK" });
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "duplicate failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <TextField
        value={name}
        label="duplicate name"
        onChange={(e) => {
          setName(e.target.value);
        }}
      />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleRename}>
          duplicate
        </Button>
        {loading && (
          <CancelButton
            handleCancel={() => {
              cancelTask(taskId.current);
              handleClose();
            }}
          />
        )}
      </Stack>
    </Stack>
  );
}

function RenameTable(props: {
  table: string;
  handleClose;
  dbName: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, dbName, reloadTables } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(table);
  const { showNotification } = useNotification();
  const handleRename = async () => {
    if (newName == table) {
      showNotification({ severity: "error", title: "name not changed" });
      return;
    }
    const confirm = await secondConfirm({
      title: "rename table",
      content: `rename table '${table}'`,
      confirmText: "Rename",
    });
    if (!confirm) {
      return;
    }
    setLoading(true);
    commands
      .renameTable(dbName, table, newName)
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "rename OK",
          message: <CopySql sql={sql} />,
        });
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "rename failure", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <TextField
        value={newName}
        label="New Name"
        onChange={(e) => {
          setNewName(e.target.value);
        }}
      />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleRename}>
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisOneTableKind(props: {
  tables: ListTable[];
  typ: RedisTableType;
  kind: DataSourceKind;
  handleReload;
  dbName;
}) {
  const { tables, typ, kind, handleReload, dbName } = props;
  return (
    <MyTreeItem
      enable={true}
      makeMenu={() => {
        return (
          <MenuList>
            <MenuRefresh refresh={handleReload} />
          </MenuList>
        );
      }}
      label=<>
        <span>{typ}</span>
      </>>
      <VirtualListLeft
        data={tables}
        makeMenu={(table) => {
          return (
            <TableMenu
              kind={kind}
              reload={handleReload}
              table={table.name}
              dbName={dbName}
              redisTableType={typ}
            />
          );
        }}
        show={(table) => {
          return (
            <>
              <span style={textEllipsisStyle}> {table.name} </span>
              <span style={textEllipsisStyle}> {table.size} </span>
            </>
          );
        }}
      />
    </MyTreeItem>
  );
}

function RedisTables() {
  const { dbName, kind } = useContext(DbnameContext);
  const [filter, setFilter] = useState("");
  const [tables, setTables] = useState<RedisListTableRet>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const max = 50000;
  const [filterDelay] = useDebounce(filter, 1000);
  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .redisListKey(dbName, filterDelay ? filterDelay : null, max)
      .then((e) => {
        setTables(e);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list redis table failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [dbName, filterDelay, showNotification]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);

  useEffect(() => {
    return () => {
      console.log("destroy this.");
    };
  }, []);
  return (
    <>
      {loading && <CircularProgress />}
      <ClearableTextField
        value={filter}
        onChange={(e) => {
          setFilter(e.target.value);
        }}
        onClear={() => {
          setFilter("");
        }}
        fullWidth
        label="key filter"
      />
      {tables && (
        <>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.String}
            tables={tables.string_tables}
            handleReload={handleReload}
            dbName={dbName}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.List}
            tables={tables.list_tables}
            handleReload={handleReload}
            dbName={dbName}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Set}
            tables={tables.set_tables}
            handleReload={handleReload}
            dbName={dbName}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Zset}
            tables={tables.zset_tables}
            handleReload={handleReload}
            dbName={dbName}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Hash}
            tables={tables.hash_tables}
            handleReload={handleReload}
            dbName={dbName}
          />
        </>
      )}
    </>
  );
}

export { TablesTree, RedisTables, RedisViewTable, AddTableView, AddTableModal };
