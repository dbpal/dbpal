import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { modalStyle, EscDownEvent, modalHandleClose } from "./Common.ts";
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

import {
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Divider,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
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
  getSupportExportKind,
  RedisTableType,
  shrinkAble,
  TabContent,
  textEllipsisStyle,
} from "./helper";
import { v4 as uuidv4 } from "uuid";
import { ExceedMaxTreeItem, MenuRefresh, ModalHeader } from "./CommonComp";
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
} from "./store.tsx";
import { commands, Field, ListTable, RedisListTableRet } from "./bindings.ts";

function TablesTree() {
  const { uuid, kind } = useContext(UuidContext);
  const [tables, setTables] = useState<ListTable[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);

  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  const tablesId = useRef(uuidv4());
  const showItems = useMemo(() => {
    return tables.filter((e) => matchPattern(e.name)).length;
  }, [tables, matchPattern]);
  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .listTable(uuid)
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
  }, [showNotification, uuid]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  let tablesText: string;

  switch (kind) {
    case DataSourceKind.CouchBase:
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
              </MenuList>
            ),
            event: e,
          });
        }}>
        <TableViewIcon />
        <span> {tablesText} </span>
        <ShowingCount showing={showItems} total={tables.length} />
        {loading && <CircularProgress />}
      </Stack>>
      {tables
        .filter((e) => matchPattern(e.name))
        .map((e, index) => {
          if (index >= maxTree) {
            return;
          }
          const id = `${uuid}-table-${e.name}-${e.rand}`;
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
      <ExceedMaxTreeItem showItems={showItems} maxTreeSize={maxTree} />
    </TreeItem>
  );
}

function TableTree(props: {
  table: string;
  reloadTables;
  size: number | null;
  id: string;
}) {
  const { table, reloadTables, size, id } = props;
  const { uuid, kind } = useContext(UuidContext);
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
                  uuid={uuid}
                />
              ),
              event: e,
            });
          }}>
          <Stack direction="row" sx={{ alignItems: "center", flex: 1, minWidth: 0 }}>
            <TableViewIcon sx={{ flexShrink: 0 }} /> {/* 图标不收缩 */}
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
  reloadTables;
  uuid: string;
  handleClose;
  kind: DataSourceKind;
}) {
  const { table, reloadTables, uuid, handleClose, kind } = props;
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
      .dropTable(uuid, table, cascade)
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
  uuid: string;
  table: string;
  redisType: RedisTableType;
}

// 1. Rename to start with an Uppercase letter
function RedisViewTable({ uuid, table, redisType }: RedisViewTableProps) {
  // 2. Using a mapping object is cleaner than a switch statement
  switch (redisType) {
    case RedisTableType.String:
      return <RedisStringTable uuid={uuid} stringKey={table} />;
    case RedisTableType.List:
      return <RedisListTable uuid={uuid} listKey={table} />;
    case RedisTableType.Set:
      return <RedisSetTable uuid={uuid} setKey={table} />;
    case RedisTableType.Zset:
      return <RedisZsetTable uuid={uuid} zsetKey={table} />;
    case RedisTableType.Hash:
      return <RedisHashTable uuid={uuid} hashKey={table} />;
    default:
      return null; // Fallback for safety
  }
}

function makeViewTable(
  uuid,
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
      tableList = <SqlTableView isTable tableName={table} uuid={uuid} kind={kind} />;
      break;
    case DataSourceKind.Redis:
      tableList = (
        <RedisViewTable uuid={uuid} table={table} redisType={redisTableType} />
      );
      break;
    case DataSourceKind.Mongo:
      tableList = <MongoTable uuid={uuid} table={table} isTable />;
      break;
    case DataSourceKind.CouchBase:
      tableList = <CouchBaseTable uuid={uuid} table={table} />;
  }
  return tableList;
}

function AddTableView(props: {
  uuid: string;
  table: string;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  const { uuid, table, kind, redisTableType } = props;

  switch (kind) {
    case DataSourceKind.Redis:
      switch (redisTableType) {
        case RedisTableType.String:
          return <RedisStringTable uuid={uuid} stringKey={table} />;
        case RedisTableType.List:
          return <RedisAddList uuid={uuid} listKey={table} />;
        case RedisTableType.Set:
          return <RedisAddSet uuid={uuid} setKey={table} />;
        case RedisTableType.Zset:
          return <RedisAddZset uuid={uuid} zsetKey={table} />;
        case RedisTableType.Hash:
          return <RedisAddHash uuid={uuid} hashKey={table} />;
      }
      break;

    case DataSourceKind.Mongo:
      return (
        <MongoTableAdd
          addOP={async (data) => {
            return commands.mongoInsert(uuid, table, BSON.serialize(data));
          }}
        />
      );
    case DataSourceKind.CouchBase:
      return <CouchBaseInsert uuid={uuid} table={table} />;
  }

  return <SqlAddTableView uuid={uuid} table={table} />;
}

function AddTableModal(props: {
  uuid: string;
  tableName: string;
  kind: DataSourceKind;
  redisType?: RedisTableType;
}) {
  const { tableName, redisType, uuid, kind } = props;
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
        <Modal
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
              uuid={uuid}
              table={tableName}
              redisTableType={redisType}
              kind={kind}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

function SqlAddTableView(props: { uuid: string; table: string }) {
  const { uuid, table } = props;
  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  useEffect(() => {
    commands
      .showTableColumns(uuid, table)
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
  }, [showNotification, table, uuid]);
  return (
    <SqlTableViewData
      isAdd
      buttonName="add"
      handleData={async (values) => {
        await commands.tableInsert(uuid, table, values);
      }}
      fields={fields}
    />
  );
}

function TableMenu(props: {
  table: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  const { table, uuid, reload, kind, redisTableType } = props;
  const [showDup, setShowDup] = useState(false);
  const [showRename, setShowRename] = useState(false);
  const [showDrop, setShowDrop] = useState(false);
  const [showExport, setShowExport] = useState(false);
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
      .truncateTable(uuid, table)
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
    const tableList: ReactNode = makeViewTable(uuid, table, kind, redisTableType);
    appendTab(new TabContent(table, tableList, <TableViewIcon />));
  };
  const handleAddData = async () => {
    appendTab(
      new TabContent(
        `${table}*`,
        <AddTableView
          uuid={uuid}
          kind={kind}
          table={table}
          redisTableType={redisTableType}
        />,
        <TableViewIcon />,
      ),
    );
  };

  const handleStructure = () => {
    appendTab(
      new TabContent(
        `${table}`,
        <TableStructure kind={kind} table={table} uuid={uuid} />,
        <IoConstructOutline />,
      ),
    );
  };

  return (
    <div>
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
            setShowDup(true);
          }}>
          <ListItemIcon>
            <HiOutlineDuplicate style={{ fontSize: "1.7rem" }} />
          </ListItemIcon>
          <ListItemText> Duplicate </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setShowExport(true);
          }}>
          <ListItemIcon>
            <ArrowCircleDownIcon />
          </ListItemIcon>
          <ListItemText> Export </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setShowRename(true);
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
            setShowDrop(true);
          }}>
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText>Drop</ListItemText>
        </MenuItem>
      </MenuList>
      {showDup && (
        <Modal
          open={showDup}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDup(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ModalHeader
              title="Duplicate"
              handleClose={() => {
                setShowDup(false);
              }}
            />
            <DupTable
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowDup(false);
              }}
              reloadTables={reload}
            />
          </Box>
        </Modal>
      )}
      {showRename && (
        <Modal
          open={showRename}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowRename(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ModalHeader
              handleClose={() => {
                setShowRename(false);
              }}
              title="Rename"
            />
            <RenameTable
              uuid={uuid}
              table={table}
              reloadTables={reload}
              handleClose={() => {
                setShowRename(false);
              }}
            />
          </Box>
        </Modal>
      )}
      {showExport && (
        <ExportTableOrView
          handleClose={() => {
            setShowExport(false);
          }}
          uuid={uuid}
          name={table}
          supportKind={getSupportExportKind(kind, true)}
        />
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDrop(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ModalHeader
              handleClose={() => {
                setShowDrop(false);
              }}
              title="Drop Table"
            />
            <DropTable
              uuid={uuid}
              table={table}
              reloadTables={reload}
              handleClose={() => {
                setShowDrop(false);
              }}
              kind={kind}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

function DupTable(props: {
  table: string;
  handleClose;
  uuid: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, uuid, reloadTables } = props;
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
      .duplicateTable(uuid, table, taskId.current, name)
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
  uuid: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, uuid, reloadTables } = props;
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
      .renameTable(uuid, table, newName)
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
  uuid;
}) {
  const { tables, typ, kind, handleReload, uuid } = props;
  let name;
  switch (typ) {
    case RedisTableType.String:
      name = "string";
      break;
    case RedisTableType.List:
      name = "list";
      break;
    case RedisTableType.Set:
      name = "set";
      break;
    case RedisTableType.Zset:
      name = "zset";
      break;
    case RedisTableType.Hash:
      name = "hash";
      break;
  }
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
        <span>{name}</span>
      </Stack>>
      {tables.map((table) => (
        <TreeItem
          key={table.name}
          itemId={uuid + "-key-" + name + "-" + table.name + table.rand}
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
                    uuid={uuid}
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
  const { uuid, kind } = useContext(UuidContext);
  const [filter, setFilter] = useState("");
  const [tables, setTables] = useState<RedisListTableRet>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const max = 50;

  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .redisListKey(uuid, filter ? filter : null, max)
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
  }, [filter, showNotification, uuid]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  useEffect(() => {
    handleReload();
  }, [filter, handleReload]);

  const [showEditFilter, setShowEditFilter] = useState(false);
  const filterTreeID = useRef(uuidv4());
  return (
    <>
      <TreeItem
        itemId={filterTreeID.current}
        label=<Stack
          direction="row"
          style={{ cursor: "pointer" }}
          onClick={() => {
            setShowEditFilter(true);
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
      {tables && (
        <>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.String}
            tables={tables.string_tables}
            handleReload={handleReload}
            uuid={uuid}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.List}
            tables={tables.list_tables}
            handleReload={handleReload}
            uuid={uuid}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Set}
            tables={tables.set_tables}
            handleReload={handleReload}
            uuid={uuid}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Zset}
            tables={tables.zset_tables}
            handleReload={handleReload}
            uuid={uuid}
          />
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Hash}
            tables={tables.hash_tables}
            handleReload={handleReload}
            uuid={uuid}
          />
        </>
      )}

      {showEditFilter && (
        <Modal
          open={showEditFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditFilter(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ModalHeader
              handleClose={() => {
                setShowEditFilter(false);
              }}
              title="Edit key match string"
            />
            <ClearableTextField
              value={filter}
              fullWidth
              label="filer"
              helperText="key filter"
              onChange={(e) => {
                setFilter(e.target.value);
              }}
              onClear={() => {
                setFilter("");
              }}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

export { TablesTree, RedisTables, RedisViewTable, AddTableView, AddTableModal };
