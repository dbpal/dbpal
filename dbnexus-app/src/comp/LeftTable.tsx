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
  AlertTitle,
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
  Slider,
  Stack,
  TextField,
  Tooltip,
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
  AppCode,
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
  useRedisKeyFilter,
  useRedisMaxKeyStore,
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
  const [showDup, setShowDup] = useState("");
  const [showExport, setShowExport] = useState("");
  const [renameTable, setRenameTable] = useState("");
  const [dropTable, setDropTable] = useState("");
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
                table={table.name}
                dbName={dbName}
                dupTable={() => {
                  setShowDup(table.name);
                }}
                exportTable={() => {
                  setShowExport(table.name);
                }}
                dropTable={() => {
                  setDropTable(table.name);
                }}
                renameTable={() => {
                  setRenameTable(table.name);
                }}
              />
            );
          }}
          table={table}
          isTable
        />
      ))}
      {showDup && (
        <DupTable
          table={showDup}
          handleClose={() => {
            setShowDup("");
          }}
          reloadTables={handleReload}
        />
      )}

      {showExport && (
        <ExportTableOrView
          isTable
          tableName={showExport}
          handleClose={() => {
            setShowExport("");
          }}
        />
      )}
      {renameTable && (
        <RenameTable
          table={renameTable}
          handleClose={() => {
            setRenameTable("");
          }}
          reloadTables={handleReload}
        />
      )}
      {dropTable && (
        <DropTable
          table={dropTable}
          handleClose={() => {
            setDropTable("");
          }}
          reloadTables={handleReload}
        />
      )}
    </MyTreeItem>
  );
}

function DropTable(props: {
  table: string;
  reloadTables: () => void;
  handleClose: () => void;
}) {
  const { dbName, kind } = useContext(DbnameContext);
  const { table, reloadTables, handleClose } = props;
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

  if (!table) {
    return;
  }
  return (
    <Dialog open onClose={handleClose}>
      <AlertTitle> drop table </AlertTitle>
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
    </Dialog>
  );
}
interface RedisViewTableProps {
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
      tableList = <RedisViewTable table={table} redisType={redisTableType} />;
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
  table: string;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  const { dbName } = useContext(DbnameContext);
  const { table, kind, redisTableType } = props;

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
      return <CouchBaseInsert table={table} />;
  }

  return <SqlAddTableView table={table} />;
}

function AddTableModal(props: {
  tableName: string;
  kind: DataSourceKind;
  redisType?: RedisTableType;
}) {
  const { tableName, redisType, kind } = props;
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
            <AddTableView table={tableName} redisTableType={redisType} kind={kind} />
          </Box>
        </Dialog>
      )}
    </>
  );
}

function SqlAddTableView(props: { table: string }) {
  const { table } = props;
  const { dbName } = useContext(DbnameContext);
  const [fields, setFields] = useState<Field[]>([]);
  const [sql, setSql] = useState("");
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

  const handleGenSql = (values) => {
    commands
      .tableInsertSql(dbName, table, values)
      .then((sql) => {
        setSql(sql);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "generate sql failed",
          message: e,
        });
      });
  };

  return (
    <Stack>
      {sql && <AppCode codeBlock={sql} language="sql" />}
      <SqlTableViewData
        isAdd
        buttonName="add"
        handleData={async (values) => {
          await commands.tableInsert(dbName, table, values);
        }}
        fields={fields}
        extraButtons={[
          {
            name: "gen sql",
            handleValue: (x) => {
              handleGenSql(x);
            },
          },
        ]}
      />
    </Stack>
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
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
  dupTable: () => void;
  exportTable: () => void;
  dropTable: () => void;
  renameTable: () => void;
}) {
  const {
    table,
    dbName,
    kind,
    redisTableType,
    dupTable,
    exportTable,
    dropTable,
    renameTable,
  } = props;
  const { showNotification } = useNotification();
  const { appendTab } = useTabsManage();

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
    const tableList: ReactNode = makeViewTable(table, kind, redisTableType);
    appendTab(new TabContent(table, tableList, <TableViewIcon />, dbName, kind));
  };
  const handleAddData = async () => {
    appendTab(
      new TabContent(
        `${table}*`,
        <AddTableView kind={kind} table={table} redisTableType={redisTableType} />,
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
          dupTable();
        }}>
        <ListItemIcon>
          <HiOutlineDuplicate style={{ fontSize: "1.7rem" }} />
        </ListItemIcon>
        <ListItemText> Duplicate </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          exportTable();
        }}>
        <ListItemIcon>
          <ArrowCircleDownIcon />
        </ListItemIcon>
        <ListItemText> Export </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          renameTable();
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
          dropTable();
        }}>
        <ListItemIcon>
          <DeleteForeverIcon color="error" />
        </ListItemIcon>
        <ListItemText>Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function DupTable(props: { table?: string; handleClose; reloadTables: () => void }) {
  const { dbName } = useContext(DbnameContext);
  const { table, handleClose, reloadTables } = props;
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
  if (!table) {
    return;
  }
  return (
    <Dialog open onClose={handleClose}>
      <AlertTitle>duplicate table </AlertTitle>
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
    </Dialog>
  );
}

function RenameTable(props: { table: string; handleClose; reloadTables: () => void }) {
  const { dbName } = useContext(DbnameContext);
  const { table, handleClose, reloadTables } = props;
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
    <Dialog open onClose={handleClose}>
      <AlertTitle>rename table </AlertTitle>
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
    </Dialog>
  );
}

function RedisOneTableKind(props: {
  tables: ListTable[];
  typ: RedisTableType;
  kind: DataSourceKind;
  handleReload;
}) {
  const { dbName } = useContext(DbnameContext);
  const { tables, typ, kind, handleReload } = props;
  const [showDup, setShowDup] = useState("");
  const [showExport, setShowExport] = useState("");
  const [renameTable, setRenameTable] = useState("");
  const [dropTable, setDropTable] = useState("");
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
              table={table.name}
              dbName={dbName}
              redisTableType={typ}
              dupTable={() => {
                setShowDup(table.name);
              }}
              exportTable={() => {
                setShowExport(table.name);
              }}
              dropTable={() => {
                setDropTable(table.name);
              }}
              renameTable={() => {
                setRenameTable(table.name);
              }}
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

      {showDup && (
        <DupTable
          table={showDup}
          handleClose={() => {
            setShowDup("");
          }}
          reloadTables={handleReload}
        />
      )}
      {showExport && (
        <ExportTableOrView
          isTable
          tableName={showExport}
          handleClose={() => {
            setShowExport("");
          }}
        />
      )}

      {renameTable && (
        <RenameTable
          table={renameTable}
          handleClose={() => {
            setRenameTable("");
          }}
          reloadTables={handleReload}
        />
      )}
      {dropTable && (
        <DropTable
          table={dropTable}
          handleClose={() => {
            setDropTable("");
          }}
          reloadTables={handleReload}
        />
      )}
    </MyTreeItem>
  );
}

function RedisTables() {
  const { dbName, kind } = useContext(DbnameContext);
  const { filters, setFilter, removeFilter } = useRedisKeyFilter();
  const filter = filters[dbName] || "";
  const [tables, setTables] = useState<RedisListTableRet>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const { maxRow, setMaxRow } = useRedisMaxKeyStore();
  const [filterDelay] = useDebounce(filter, 1000);
  const [maxRowDelay] = useDebounce(maxRow, 1000);
  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .redisListKey(dbName, filterDelay ? filterDelay : null, maxRowDelay)
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
  }, [dbName, filterDelay, maxRowDelay, showNotification]);

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
      <Stack
        style={{
          paddingLeft: "0.5rem",
        }}>
        <ClearableTextField
          value={filter}
          onChange={(e) => {
            setFilter(dbName, e.target.value);
          }}
          onClear={() => {
            removeFilter(dbName);
          }}
          fullWidth
          label="key filter"
        />
        <Stack direction="row" style={{ alignItems: "center" }}>
          <Tooltip
            title={(function () {
              const d = "limit the max key can return.";
              if (!tables) {
                return d;
              }
              return tables.has_more ? "some key not return because of this limit" : d;
            })()}>
            <Slider
              value={maxRow}
              onChange={(_, value) => {
                setMaxRow(value);
              }}
              style={{ flex: 1 }}
              step={1000}
              min={10000}
              max={100_0000}
              color={tables && tables.has_more ? "warning" : "success"}
            />
          </Tooltip>
        </Stack>
        {loading && <CircularProgress />}
      </Stack>

      {tables && (
        <>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.String}
            tables={tables.string_tables}
            handleReload={handleReload}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.List}
            tables={tables.list_tables}
            handleReload={handleReload}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Set}
            tables={tables.set_tables}
            handleReload={handleReload}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Zset}
            tables={tables.zset_tables}
            handleReload={handleReload}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Hash}
            tables={tables.hash_tables}
            handleReload={handleReload}
          />
        </>
      )}
    </>
  );
}

export { TablesTree, RedisTables, RedisViewTable, AddTableView, AddTableModal };
