import { ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import { modalStyle, tabsHasId, EscDownEvent, modalHandleClose } from "./Common.ts";
import types from "./ApiStruct";
import { NotificationContext } from "./Notification";
import { invoke } from "@tauri-apps/api/core";
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
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  TextField,
  Tooltip,
} from "@mui/material";
import ClearIcon from "@mui/icons-material/Clear";

import { TabContent } from "./Tabs";
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
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  MatchPatternContext,
  TabsSetContext,
  UuidContext,
  handleCopyToClipboard,
  getSupportExportKind,
  RedisTableType,
  TableType,
  shrinkAble,
  MaxTreeContext,
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh, ModalHeader } from "./CommonComp";
import { TableStructure } from "./TableStructure";
import { cancelTask, listTable, makeTableInsertFn, showTableColumn } from "./handlers";
import { filesize } from "filesize";
import { CancelButton, CopySql, StackRowCenter } from "./CommonComp2";
import { CouchBaseTable } from "./CouchBase";

function TablesTree(props: { typ: TableType }) {
  const { typ } = props;
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [tables, setTables] = useState<types.ListTable[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  const maxsize = useContext(MaxTreeContext);
  useEffect(() => {
    setExceed(tables.filter((e) => matchPattern(e.name)).length);
  }, [tables, matchPattern]);
  const handleReload = useCallback(() => {
    setLoading(true);
    listTable(uuid, typ)
      .then((ts) => {
        setTables(ts);
      })
      .catch((e) => {
        showNotification("error", "list table failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, typ, uuid]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  const { setUpMenu } = useContext(ContextMenuContext);

  let tablesText;

  switch (kind) {
    case DataSourceKind.CouchBase:
      tablesText = "collections";
      break;
    default: {
      switch (typ) {
        case TableType.Normal:
          tablesText = "tables";
          break;
        case TableType.MongoTimeSerial:
          tablesText = "time serial";
          break;
      }
    }
  }

  const tablesId = useRef(generateRandomString());
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
        }}
      >
        <TableViewIcon />
        <span> {tablesText} </span>
        {loading && <CircularProgress />}
      </Stack>
    >
      {tables
        .filter((e) => matchPattern(e.name))
        .map((e, index) => {
          if (index >= maxsize) {
            return;
          }
          const id = `${uuid}-table-${typ}-${index}`;

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
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxsize} />
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
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={id}
      label={
        <Stack
          direction="row"
          justifyContent="space-between"
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
          }}
        >
          <StackRowCenter>
            <TableViewIcon> </TableViewIcon> <span>{table}</span>
          </StackRowCenter>
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
  const { showNotification } = useContext(NotificationContext);
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
    invoke("drop_table", { name: uuid, tableName: table, cascade })
      .then((sql) => {
        showNotification("success", "Drop table OK", <CopySql sql={sql} />);
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification("error", "Drop table failure", e);
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

function makeRedisViewTable(uuid, table, redisType: RedisTableType) {
  let tableList;
  switch (redisType) {
    case RedisTableType.String:
      tableList = <RedisStringTable uuid={uuid} stringKey={table} />;
      break;
    case RedisTableType.List:
      tableList = <RedisListTable uuid={uuid} listKey={table} />;
      break;

    case RedisTableType.Set:
      tableList = <RedisSetTable setKey={table} uuid={uuid} />;
      break;
    case RedisTableType.Zset:
      tableList = <RedisZsetTable zsetKey={table} uuid={uuid} />;
      break;
    case RedisTableType.Hash:
      tableList = <RedisHashTable hashKey={table} uuid={uuid} />;
      break;
  }
  return tableList;
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
      tableList = makeRedisViewTable(uuid, table, redisTableType);
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
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
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
            return invoke<void>("mongo_insert", {
              name: uuid,
              tableName: table,
              doc: BSON.serialize(data),
            });
          }}
        />
      );
  }

  // eslint-disable-next-line react-hooks/rules-of-hooks
  useEffect(() => {
    showTableColumn(uuid, table)
      .then((e) => {
        setFields(e);
      })
      .catch((e) => {
        showNotification("error", "list fields failed", e);
      });
  }, [showNotification, table, uuid]);

  return (
    <SqlTableViewData
      isAdd
      buttonName="add"
      handleData={makeTableInsertFn(uuid, table)}
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

  const { showNotification } = useContext(NotificationContext);
  const { tabs, setSelectTab, appendTab } = useContext(TabsSetContext);

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
    invoke("truncate_table", { tableName: table, name: uuid })
      .then((sql) => {
        showNotification("success", `Truncate table OK`, <CopySql sql={sql} />);
      })
      .catch((e) => {
        showNotification("error", `Truncate  table failure`, e);
      });
  };

  const handleViewData = () => {
    const id = uuid + "-table-" + generateRandomString(10);
    const tableList: ReactNode = makeViewTable(uuid, table, kind, redisTableType);
    appendTab(
      new TabContent(id, table, tableList, <TableViewIcon className="tab-icon-size" />),
    );
  };
  const handleAddData = async () => {
    const id = generateRandomString(10);

    appendTab(
      new TabContent(
        id,
        `${table}*`,
        <AddTableView
          uuid={uuid}
          kind={kind}
          table={table}
          redisTableType={redisTableType}
        />,
        <TableViewIcon className="tab-icon-size" />,
      ),
    );
  };

  const handleStructure = () => {
    const id = uuid + "-table-" + table + "-struct";
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setSelectTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          `${table}`,
          <TableStructure kind={kind} table={table} uuid={uuid} />,
          <IoConstructOutline className="tab-icon-size" />,
        ),
      );
    }
  };

  return (
    <div>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleViewData();
          }}
        >
          <ListItemIcon>
            <ListIcon />
          </ListItemIcon>
          <ListItemText> View Data</ListItemText>
        </MenuItem>

        {redisTableType != RedisTableType.String && (
          <MenuItem
            onClick={() => {
              handleAddData();
            }}
          >
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
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy Name </ListItemText>
        </MenuItem>

        <Divider />

        <MenuItem
          onClick={() => {
            setShowDup(true);
          }}
        >
          <ListItemIcon>
            <HiOutlineDuplicate style={{ fontSize: "1.7rem" }} />
          </ListItemIcon>
          <ListItemText> Duplicate </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setShowExport(true);
          }}
        >
          <ListItemIcon>
            <ArrowCircleDownIcon />
          </ListItemIcon>
          <ListItemText> Export </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setShowRename(true);
          }}
        >
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
          }}
        >
          <ListItemIcon>
            <PlaylistRemoveIcon color="error" />
          </ListItemIcon>
          <ListItemText> Truncate </ListItemText>
        </MenuItem>
        <MenuItem
          sx={{ color: "error.main" }}
          onClick={() => {
            setShowDrop(true);
          }}
        >
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
          })}
        >
          <Box sx={modalStyle} width="45%">
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
          })}
        >
          <Box sx={modalStyle} width="45%">
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
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ExportTableOrView
              handleClose={() => {
                setShowExport(false);
              }}
              uuid={uuid}
              name={table}
              supportKind={getSupportExportKind(kind, true)}
            />
          </Box>
        </Modal>
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDrop(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
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
  const { showNotification } = useContext(NotificationContext);
  const taskId = useRef(generateRandomString());
  const handleRename = () => {
    if (name == table) {
      showNotification("error", "name not changed");
      return;
    }
    setLoading(true);
    invoke("duplicate_table", {
      name: uuid,
      from: table,
      to: name,
      taskId: taskId.current,
    })
      .then(() => {
        showNotification("success", "duplicate OK");
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification("error", "duplicate failure", e);
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
  const { showNotification } = useContext(NotificationContext);
  const handleRename = async () => {
    if (newName == table) {
      showNotification("error", "name not changed");
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
    invoke("rename_table", { name: uuid, old: table, new: newName })
      .then((sql) => {
        showNotification("success", "rename OK", <CopySql sql={sql} />);
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification("error", "rename failure", e);
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
  tables: string[];
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
  const tablesID = useRef(generateRandomString());
  const { setUpMenu } = useContext(ContextMenuContext);
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
        }}
      >
        <span>{name}</span>
      </Stack>
    >
      {tables.map((table) => (
        <TreeItem
          key={table}
          itemId={uuid + "-key-" + name + "-" + table}
          label=<Stack
            direction="row"
            onContextMenu={(e) => {
              setUpMenu({
                content: (
                  <TableMenu
                    kind={kind}
                    reload={handleReload}
                    table={table}
                    uuid={uuid}
                    redisTableType={typ}
                  />
                ),
                event: e,
              });
            }}
          >
            <span> {table} </span>
          </Stack>
        />
      ))}
    </TreeItem>
  );
}

function RedisTables() {
  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const [filter, setFilter] = useState("");
  const [tables, setTables] = useState<types.RedisListTable>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const max = 50;

  const handleReload = useCallback(() => {
    setLoading(true);
    invoke<types.RedisListTable>("redis_list_table", {
      name: uuid,
      filter: filter ? filter : null,
      max,
    })
      .then((e) => {
        setTables(e);
      })
      .catch((e) => {
        showNotification("error", "list redis table failed", e);
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
  const filterTreeID = useRef(generateRandomString());
  return (
    <>
      <TreeItem
        itemId={filterTreeID.current}
        label=<Stack
          direction="row"
          style={{ cursor: "pointer" }}
          onClick={() => {
            setShowEditFilter(true);
          }}
        >
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
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              handleClose={() => {
                setShowEditFilter(false);
              }}
              title="Edit key match string"
            />
            <TextField
              value={filter}
              fullWidth
              label="filer"
              helperText="key filter"
              onChange={(e) => {
                setFilter(e.target.value);
              }}
              slotProps={{
                input: {
                  endAdornment: filter && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setFilter("");
                        }}
                        edge="end"
                      >
                        <ClearIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

export { TablesTree, RedisTables, makeRedisViewTable, AddTableView };
