import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { modalStyle, EscDownEvent, modalHandleClose } from "./Common";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import { IoConstructOutline } from "react-icons/io5";
import {
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Typography,
} from "@mui/material";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import VisibilityIcon from "@mui/icons-material/Visibility";
import ViewComfyAltIcon from "@mui/icons-material/ViewComfyAlt";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import ListIcon from "@mui/icons-material/List";
import { SqlTableView } from "./SqlTableView";
import { ExportTableOrView } from "./Export";
import {
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  getSupportExportKind,
  handleCopyToClipboard,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  UuidContext,
  ViewKind,
} from "./helper";
import { v4 as uuidv4 } from "uuid";
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { MongoTable } from "./MongoDB";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "@mui/x-data-grid";
import { AppDataGridPagination, CopySql, ShowingCount } from "./CommonComp2";
import { listView } from "./handlers";
import { filesize } from "filesize";
import {
  useMatcher,
  useContextMenu,
  useNotification,
  useTabsManage,
  useColumnWidthStore,
  useUserSettings,
  useMatcherFn,
} from "./store";
import { AllKindChat } from "./Chat";
import { commands, Field, ListTable } from "./bindings";
function ViewsTree(props: { viewKind: ViewKind }) {
  const { viewKind } = props;
  const { uuid } = useContext(UuidContext);
  const [views, setViews] = useState<ListTable[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);
  const [showItems, setShowItems] = useState(0);
  const { maxTree } = useMatcher();
  const matchPattern = useMatcherFn();
  useEffect(() => {
    setShowItems(views.filter((e) => matchPattern(e.name)).length);
  }, [views, matchPattern]);
  const handleReload = useCallback(() => {
    setLoading(true);
    listView(uuid, viewKind)
      .then((ts) => {
        setViews(ts);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list view failed", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, uuid, viewKind]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  let name;
  const icon = getIcon(viewKind);
  switch (viewKind) {
    case ViewKind.View:
      name = "views";
      break;
    case ViewKind.MaterializedView:
      name = "materialized views";
      break;
  }
  const tressId = useRef(uuidv4());
  return (
    <TreeItem
      itemId={tressId.current}
      label=<Stack
        direction="row"
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
        {icon}
        <span> {name} </span>
        <ShowingCount showing={showItems} total={views.length} />
        {loading && <CircularProgress />}
      </Stack>>
      {views
        .filter((e) => matchPattern(e.name))
        .map((e, index) => {
          if (index >= maxTree) {
            return;
          }
          return (
            <ViewTree
              viewKind={viewKind}
              key={e.name}
              view={e.name}
              reload={handleReload}
              size={e.size}
            />
          );
        })}
      <ExceedMaxTreeItem showItems={showItems} maxTreeSize={maxTree} />
    </TreeItem>
  );
}

function StatementStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  view: string;
  viewKind: ViewKind;
}) {
  const { uuid, kind, view, viewKind } = props;
  const [sql, setSql] = useState("");
  const [loading, setLoading] = useState(false);
  const { showNotification } = useNotification();
  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .createViewSql(uuid, view, viewKind == ViewKind.MaterializedView)
      .then((e) => {
        setSql(e);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "fetch create sql failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, uuid, view, viewKind]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row">
        <Button
          loading={loading}
          onClick={() => {
            handleReload();
          }}>
          reload
        </Button>
      </Stack>
      {sql && <AllKindChat uuid={uuid} kind={kind} value={sql} />}
    </Stack>
  );
}

function ViewStructure(props: {
  uuid: string;
  view: string;
  viewKind: ViewKind;
  kind;
}) {
  const { uuid, view, viewKind, kind } = props;

  const [structure, setStructure] = useState("column");
  const structures = ["column"];

  const supportStatement = [
    DataSourceKind.LibSql,
    DataSourceKind.Oracle,
    DataSourceKind.Mysql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Sqlite,
  ].includes(kind);

  if (supportStatement) {
    structures.push("statement");
  }

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ justifyContent: "center" }}>
        <Stack direction="row" sx={{ justifyContent: "center" }}>
          <ButtonGroup>
            {structures.map((e) => {
              return (
                <Button
                  disabled={structure == e}
                  onClick={() => {
                    setStructure(e);
                  }}
                  variant="contained"
                  key={e}>
                  {e}
                </Button>
              );
            })}
          </ButtonGroup>
        </Stack>
      </Stack>
      {structure == "column" && <ColumnsContent uuid={uuid} view={view} />}
      {structure == "statement" && (
        <StatementStructure uuid={uuid} kind={kind} viewKind={viewKind} view={view} />
      )}
    </Stack>
  );
}

function ColumnsMenu(props: { content: string }) {
  const { content } = props;
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
    </MenuList>
  );
}

function ColumnsContent(props: { uuid: string; view: string }) {
  const { uuid, view } = props;
  const setting = useUserSettings();

  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);

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
      .showViewColumns(uuid, view)
      .then((fs) => {
        setFields(fs);
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
  }, [showNotification, uuid, view]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [
      { field: "field" },
      {
        field: "data_type",
        renderHeader: () => {
          return "data type";
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
          {
            const field = extractFieldAndIDFromMouseEvent(e).field;
            if (!field) {
              return;
            }
          }
          const content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: <ColumnsMenu content={content} />,
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
  }, [setUpMenu, setting.export_.csvDelimiter, setting.export_.includeHeader]);

  const getRowId = useCallback((e) => {
    return e["field"];
  }, []);
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
      </Stack>
      <DataGrid
        slots={slots}
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        columns={cols}
        rows={fields}
        style={dataGridStyle}
        loading={loading}
      />
    </Stack>
  );
}

function getIcon(viewKind: ViewKind) {
  switch (viewKind) {
    case ViewKind.MaterializedView:
      return <ViewComfyAltIcon />;
    default:
      return <VisibilityIcon />;
  }
}
function ViewTree(props: { view: string; reload; viewKind: ViewKind; size?: number }) {
  const { view, reload, viewKind, size } = props;
  const { uuid, kind } = useContext(UuidContext);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const icon = getIcon(viewKind);

  return (
    <TreeItem
      itemId={uuid + "-view-" + viewKind + "-" + view}
      label={
        <Stack
          direction="row"
          sx={{ justifyContent: "space-between" }}
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <ViewMenu
                  kind={kind}
                  reload={reload}
                  view={view}
                  uuid={uuid}
                  viewKind={viewKind}
                />
              ),
              event: e,
            });
          }}>
          <Stack direction="row" style={{ alignItems: "center", flex: 1, minWidth: 0 }}>
            {icon} <span style={textEllipsisStyle}> {view} </span>
          </Stack>
          {viewKind == ViewKind.MaterializedView && size && (
            <span> {filesize(size, { round: 0 })} </span>
          )}
        </Stack>
      }
    />
  );
}

function ViewMenu(props: {
  view: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
  viewKind: ViewKind;
}) {
  const { view, uuid, reload, kind, viewKind } = props;
  const { showNotification } = useNotification();
  const [showExport, setShowExport] = useState(false);
  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop view '${view}'`,
      content: "",
      confirmText: "Drop",
    });
    if (!confirm) {
      return;
    }

    (viewKind == ViewKind.MaterializedView
      ? commands.dropMaterializedView(uuid, view)
      : commands.dropView(uuid, view)
    )
      .then((sql) => {
        showNotification({
          severity: "success",
          title: "Drop View OK",
          message: <CopySql sql={sql} />,
        });
        reload();
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "Drop View Failure", message: e });
      });
  };
  const { appendTab } = useTabsManage();
  const handleViewData = () => {
    let list: ReactNode = null;
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
        list = (
          <SqlTableView
            isTable={false}
            tableName={view}
            kind={kind}
            uuid={uuid}
            notWarnPri={true}
          />
        );
        break;

      case DataSourceKind.Mongo:
        list = <MongoTable isTable={false} uuid={uuid} table={view} />;
        break;
      default:
        showNotification({
          severity: "error",
          title: `data source ${kind} need impl view`,
        });
    }

    appendTab(new TabContent(view, list, <VisibilityIcon />));
  };

  const handleStructure = () => {
    appendTab(
      new TabContent(
        `${view}`,
        <ViewStructure view={view} uuid={uuid} kind={kind} viewKind={viewKind} />,
        <IoConstructOutline />,
      ),
    );
  };

  return (
    <div>
      <MenuList>
        <MenuItem onClick={handleViewData}>
          <ListItemIcon>
            <ListIcon />
          </ListItemIcon>
          <ListItemText> View Data</ListItemText>
          <Typography variant="body2" sx={{ color: "text.secondary" }} />
        </MenuItem>

        <MenuItem onClick={handleStructure}>
          <ListItemIcon>
            <IoConstructOutline style={{ fontSize: "1.2rem" }} />
          </ListItemIcon>
          <ListItemText> Structure </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(view, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy Name </ListItemText>
        </MenuItem>

        <Divider />
        <MenuItem
          onClick={() => {
            setShowExport(true);
          }}>
          <ListItemIcon>
            <ArrowCircleDownIcon />
          </ListItemIcon>
          <ListItemText> Export </ListItemText>
        </MenuItem>
        <Divider />
        <MenuItem
          sx={{ color: "error.main" }}
          onClick={() => {
            handleDrop();
          }}>
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText>Drop </ListItemText>
          <Typography variant="body2" sx={{ color: "text.secondary" }} />
        </MenuItem>
      </MenuList>

      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}>
          <Box sx={{ ...modalStyle, width: "45%" }}>
            <ExportTableOrView
              uuid={uuid}
              name={view}
              supportKind={getSupportExportKind(kind, false)}
              handleClose={() => {
                setShowExport(false);
              }}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

export { ViewsTree, ViewKind };
