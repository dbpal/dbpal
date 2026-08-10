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
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import ListIcon from "@mui/icons-material/List";
import EditIcon from "@mui/icons-material/Edit";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import PlaylistRemoveIcon from "@mui/icons-material/PlaylistRemove";
import TableViewIcon from "@mui/icons-material/TableView";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import { IoConstructOutline } from "react-icons/io5";
import { HiOutlineDuplicate } from "react-icons/hi";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import UploadIcon from "@mui/icons-material/Upload";
import CreateIcon from "@mui/icons-material/Create";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Dialog,
  DialogTitle,
  Divider,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
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
  UuidContext,
  handleCopyToClipboard,
  RedisTableType,
  shrinkAble,
  TabContent,
  textEllipsisStyle,
} from "./helper";
import { MenuRefresh, ModalHeader } from "./CommonComp";
import { TableStructure } from "./TableStructure";
import { cancelTask } from "./handlers";
import { filesize } from "filesize";
import {
  CancelButton,
  ClearableTextField,
  CopySql,
  ListenElementSize,
  ShowingCount,
  TableAddButton,
} from "./CommonComp2";
import { CouchBaseInsert, CouchBaseTable } from "./CouchBase";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useMatcherFn,
  useOneShotDialogUI,
} from "./store.tsx";
import { commands, Field, ListTable, RedisListTableRet } from "./bindings.ts";
import {
  CouchbaseCreateTableImportFile,
  MongoCreateTableImportFile,
  SqlCreateTable,
  SqlCreateTableImportFile,
} from "./CreateTable.tsx";

function TablesTree() {
  const { dbName, kind } = useContext(UuidContext);
  const [tables, setTables] = useState<ListTable[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);
  const { appendTab } = useTabsManage();
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const tablesId = useRef(uuidv4());
  const showItems = useMemo(() => {
    return tables.filter((e) => matchPattern(e.name)).slice(0, maxTree);
  }, [tables, maxTree, matchPattern]);
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

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

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
    <TreeItem
      itemId={tablesId.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: "center" }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={handleReload} />

                {kind != DataSourceKind.Redis && (
                  <>
                    {kind != DataSourceKind.Mongo &&
                      kind != DataSourceKind.CouchBase && (
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
                          content = <MongoCreateTableImportFile />;
                        }
                        if (kind == DataSourceKind.CouchBase) {
                          content = <CouchbaseCreateTableImportFile />;
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
            ),
            event: e,
          });
        }}>
        <TableViewIcon />
        <span> {tablesText} </span>
        <ShowingCount showing={showItems.length} total={tables.length} />
        {loading && <CircularProgress />}
      </Stack>>
      {showItems.map((e) => {
        const id = `${dbName}-table-${e.name}-${e.rand}`;
        return (
          <TableTree
            key={e.name}
            size={e.size}
            reloadTables={handleReload}
            table={e.name}
            id={id}
          />
        );
      })}
    </TreeItem>
  );
}

function TableTree(props: {
  table: string;
  reloadTables: () => void;
  size: number | null;
  id: string;
}) {
  const { table, reloadTables, size, id } = props;
  const { dbName, kind } = useContext(UuidContext);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  return (
    <TreeItem
      itemId={id}
      label={
        <Stack
          direction="row"
          sx={{ justifyContent: "space-between" }}
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <TableMenu
                  kind={kind}
                  reload={reloadTables}
                  table={table}
                  dbName={dbName}
                />
              ),
              event: e,
            });
          }}>
          <Stack direction="row" sx={{ alignItems: "center", flex: 1, minWidth: 0 }}>
            <TableViewIcon />
            <span style={textEllipsisStyle}>{table}</span>
          </Stack>{" "}
          {size && (
            <span style={{ ...shrinkAble, minWidth: 0 }}>
              {filesize(size, { round: 0 })}
            </span>
          )}
        </Stack>
      }
    />
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
      <TableAddButton handleAdd={() => setShowAdd(true)} />

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
  const { setUi } = useOneShotDialogUI();

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
          setUi(
            <DupTable
              table={table}
              dbName={dbName}
              handleClose={() => {
                setUi(null);
              }}
              reloadTables={reload}
            />,
            "Duplicate table",
          );
        }}>
        <ListItemIcon>
          <HiOutlineDuplicate style={{ fontSize: "1.7rem" }} />
        </ListItemIcon>
        <ListItemText> Duplicate </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setUi(
            <ExportTableOrView dbName={dbName} name={table} kind={kind} isTable />,
            `export table ${table}`,
          );
        }}>
        <ListItemIcon>
          <ArrowCircleDownIcon />
        </ListItemIcon>
        <ListItemText> Export </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setUi(
            <RenameTable
              table={table}
              dbName={dbName}
              handleClose={() => {
                setUi(null);
              }}
              reloadTables={reload}
            />,
            `rename table ${table}`,
          );
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
          setUi(
            <DropTable
              table={table}
              dbName={dbName}
              handleClose={() => {
                setUi(null);
              }}
              reloadTables={reload}
              kind={kind}
            />,
            `drop table ${table}`,
          );
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
  const tablesID = useRef(uuidv4());
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  return (
    <TreeItem
      itemId={tablesID.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: "center" }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={handleReload} />
              </MenuList>
            ),
            event: e,
          });
        }}>
        <span>{typ}</span>
      </Stack>>
      {tables.map((table) => (
        <TreeItem
          key={table.name}
          itemId={dbName + "-key-" + name + "-" + table.name + table.rand}
          label=<Stack
            style={{
              justifyContent: "space-between",
              alignItems: "center",
            }}
            direction="row"
            onContextMenu={(e) => {
              setUpMenu({
                content: (
                  <TableMenu
                    kind={kind}
                    reload={handleReload}
                    table={table.name}
                    dbName={dbName}
                    redisTableType={typ}
                  />
                ),
                event: e,
              });
            }}>
            <Stack style={{ flexShrink: 1 }} direction="row">
              <span style={textEllipsisStyle}> {table.name} </span>
              &nbsp;
            </Stack>

            {table.size && (
              <Tooltip
                title={(function () {
                  switch (typ) {
                    case RedisTableType.String:
                      return "";
                    case RedisTableType.List:
                      return `a list of ${table.size}`;
                    case RedisTableType.Set:
                      return `set with ${table.size} elements`;
                    case RedisTableType.Zset:
                      return `zset with ${table.size} elements`;
                    case RedisTableType.Hash:
                      return `hash with ${table.size} elements`;
                  }
                })()}>
                <span style={textEllipsisStyle}> {table.size} </span>
              </Tooltip>
            )}
          </Stack>
        />
      ))}
    </TreeItem>
  );
}

function RedisTables() {
  const { dbName, kind } = useContext(UuidContext);
  const [filter, setFilter] = useState("");
  const [tables, setTables] = useState<RedisListTableRet>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const max = 50;

  const [showEditFiler, setShowEditFiler] = useState(false);

  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .redisListKey(dbName, filter ? filter : null, max)
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
  }, [filter, showNotification, dbName]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  useEffect(() => {
    handleReload();
  }, [filter, handleReload]);

  const filterTreeID = useRef(uuidv4());
  return (
    <>
      <TreeItem
        itemId={filterTreeID.current}
        label=<Stack
          direction="row"
          style={{ cursor: "pointer" }}
          onClick={() => {
            setShowEditFiler(true);
          }}>
          {tables && tables.has_more ? (
            <Tooltip title={`result exceed ${max},some result are omitted`}>
              <Chip
                color="error"
                icon={<FilterAltIcon />}
                label={`${filter}`}
                variant="outlined"
              />
            </Tooltip>
          ) : (
            <Chip icon={<FilterAltIcon />} label={`${filter}`} />
          )}
          {loading && <CircularProgress />}
        </Stack>
      />

      {showEditFiler && (
        <Dialog
          open={true}
          onClose={() => {
            setShowEditFiler(false);
          }}>
          <DialogTitle>edit key filter</DialogTitle>
          <ClearableTextField
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
            }}
            style={{ width: "17rem" }}
            onClear={() => {
              setFilter("");
            }}
            label="filter"
          />
        </Dialog>
      )}
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
