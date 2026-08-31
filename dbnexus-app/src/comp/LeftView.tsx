import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import { IoConstructOutline } from "react-icons/io5";
import {
  Button,
  ButtonGroup,
  CircularProgress,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
  Typography,
} from "@mui/material";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import VisibilityIcon from "@mui/icons-material/Visibility";
import ViewComfyAltIcon from "@mui/icons-material/ViewComfyAlt";
import ListIcon from "@mui/icons-material/List";
import { SqlTableView } from "./SqlTableView";
import { ExportTableOrView } from "./Export";
import {
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  secondConfirm,
  TabContent,
  textEllipsisStyle,
  DbnameContext,
  ViewKind,
} from "./helper";
import { MenuRefresh, MyTreeItem, VirtualListLeft } from "./CommonComp";
import { MongoTable } from "./MongoDB";

import {
  AppDataGridPagination,
  AppDataGridToolBar,
  CopySql,
  RefreshButton,
  ShowingCount,
} from "./CommonComp2";
import { listView } from "./handlers";
import { filesize } from "filesize";
import {
  useContextMenu,
  useNotification,
  useTabsManage,
  useColumnWidthStore,
  useMatcherFn,
  useOneShotDialogUI,
} from "./store";
import { commands, DefAndMetas, Field, ListTable } from "./bindings";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "./datagridExport";
import { DefAndMetasView } from "./DefAndMetaView";
function ViewsTree(props: { viewKind: ViewKind }) {
  const { viewKind } = props;
  const { dbName, kind } = useContext(DbnameContext);
  const [views, setViews] = useState<ListTable[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);

  const matchPattern = useMatcherFn();
  const showItems = useMemo(() => {
    return views.filter((e) => matchPattern(e.name));
  }, [views, matchPattern]);

  const handleReload = useCallback(() => {
    setLoading(true);
    listView(dbName, viewKind)
      .then((ts) => {
        setViews(ts);
      })
      .catch((e) => {
        showNotification({ severity: "error", title: "list view failed", message: e });
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, dbName, viewKind]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  let name = "";
  const icon = getIcon(viewKind);
  switch (viewKind) {
    case ViewKind.View:
      name = "views";
      break;
    case ViewKind.MaterializedView:
      name = "materialized views";
      break;
  }
  return (
    <MyTreeItem
      conn_established={true}
      makeMenu={() => {
        return (
          <MenuList>
            <MenuRefresh refresh={handleReload} />
          </MenuList>
        );
      }}
      label=<>
        {icon}
        <span> {name} </span>
        <ShowingCount showing={showItems.length} total={views.length} />
        {loading && <CircularProgress />}
      </>>
      <VirtualListLeft
        data={showItems}
        makeMenu={(view) => {
          return (
            <ViewMenu
              kind={kind}
              reload={handleReload}
              view={view.name}
              dbName={dbName}
              viewKind={viewKind}
            />
          );
        }}
        show={(view) => {
          return (
            <>
              <Stack
                direction="row"
                style={{ alignItems: "center", flex: 1, minWidth: 0 }}>
                {icon} <span style={textEllipsisStyle}> {view.name} </span>
              </Stack>
              {viewKind == ViewKind.MaterializedView && view.size && (
                <span> {filesize(view.size, { round: 0 })} </span>
              )}
            </>
          );
        }}
      />
    </MyTreeItem>
  );
}

function StatementStructure(props: { view: string; viewKind: ViewKind }) {
  const { dbName } = useContext(DbnameContext);
  const { view, viewKind } = props;
  return (
    <Stack style={{ height: "100%" }}>
      <DefAndMetasView
        reloadOP={() => {
          return commands
            .createViewSql(dbName, view, viewKind == ViewKind.MaterializedView)
            .then((sql): DefAndMetas => {
              return { def: sql, metas: [] };
            });
        }}
      />
    </Stack>
  );
}

function ViewStructure(props: { view: string; viewKind: ViewKind }) {
  const { kind } = useContext(DbnameContext);
  const { view, viewKind } = props;

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
      {structure == "column" && <ColumnsContent view={view} />}
      {structure == "statement" && (
        <StatementStructure viewKind={viewKind} view={view} />
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

function ColumnsContent(props: { view: string }) {
  const { view } = props;
  const { dbName } = useContext(DbnameContext);

  const [fields, setFields] = useState<Field[]>([]);
  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);

  const { getColumnWidth } = useColumnWidthStore();

  const handleReload = useCallback(() => {
    setLoading(true);
    commands
      .showViewColumns(dbName, view)
      .then((fs) => {
        fs.forEach((e, index) => {
          e["id"] = index;
        });
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
  }, [showNotification, dbName, view]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo((): GridColDef[] => {
    return [
      { field: "field" },
      {
        field: "data_type",
        renderHeader: () => {
          return "data type";
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
    };
  }, [setUpMenu]);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName={view}
            left=<RefreshButton loading={loading} onClick={handleReload} />
          />
        );
      },
      pagination: AppDataGridPagination,
    };
  }, [handleReload, loading, view]);
  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        slots={slots}
        slotProps={slotProps}
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

function ViewMenu(props: {
  view: string;
  reload: () => void;
  viewKind: ViewKind;
  dbName: string;
  kind: DataSourceKind;
}) {
  const { view, reload, viewKind, dbName, kind } = props;
  const { showNotification } = useNotification();

  const { setOneShotUI } = useOneShotDialogUI();
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
      ? commands.dropMaterializedView(dbName, view)
      : commands.dropView(dbName, view)
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
      case DataSourceKind.Mongo:
        list = <MongoTable isTable={false} table={view} />;
        break;
      default:
        list = <SqlTableView isTable={false} tableName={view} />;
        break;
    }

    appendTab(new TabContent(view, list, <VisibilityIcon />, dbName, kind));
  };

  const handleStructure = () => {
    appendTab(
      new TabContent(
        `${view}`,
        <ViewStructure view={view} viewKind={viewKind} />,
        <IoConstructOutline />,
        dbName,
        kind,
      ),
    );
  };

  return (
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
          setOneShotUI({
            ui: (
              <ExportTableOrView
                dbName={dbName}
                name={view}
                isTable={false}
                kind={kind}
              />
            ),
            title: `export view ${view}`,
          });
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
  );
}

export { ViewsTree, ViewKind };
