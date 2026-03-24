import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  tabsHasId,
  modalStyle,
  EscDownEvent,
  modalHandleClose,
  SetAutoCompletionName,
  NameAndQueryId,
} from "./Common";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import { NotificationContext } from "./Notification";
import { invoke } from "@tauri-apps/api/core";
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
import types from "./ApiStruct";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import ListIcon from "@mui/icons-material/List";
import { SqlTableView } from "./SqlTableView";
import { TabContent } from "./Tabs";
import { ExportTableOrView } from "./Export";
import {
  ColumnWidth,
  ConnTypeContext,
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  generateRandomString,
  getLanguageFromDataSourceKind,
  getSupportExportKind,
  handleCopyToClipboard,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
  ViewKind,
} from "./helper";
import { ExceedMaxTreeItem, MenuRefresh } from "./CommonComp";
import { MongoTable } from "./MongoDB";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
} from "@mui/x-data-grid";
import { AppDataGridPagination, SimpleFlexEditor } from "./CommonComp2";
import { createViewSql, listView, showViewColumns } from "./handlers";
import { filesize } from "filesize";
import { useAppConfigStore, useContextMenu } from "./store";
function ViewsTree(props: { viewKind: ViewKind }) {
  const { viewKind } = props;
  const { uuid } = useContext(UuidContext);
  const [views, setViews] = useState<types.ListTable[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [exceed, setExceed] = useState(0);
  const { matchPattern, maxTree } = useAppConfigStore((state) => state);
  useEffect(() => {
    setExceed(views.filter((e) => matchPattern(e.name)).length);
  }, [views, matchPattern]);
  const handleReload = useCallback(() => {
    setLoading(true);
    listView(uuid, viewKind)
      .then((ts) => {
        setViews(ts);
      })
      .catch((e) => {
        showNotification("error", "list view failed", e);
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
  const tressId = useRef(generateRandomString());
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
        }}
      >
        {icon}
        <span> {name} </span>
        {loading && <CircularProgress />}
      </Stack>
    >
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
      <ExceedMaxTreeItem exceed={exceed} maxTreeSize={maxTree} />
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
  const { showNotification } = useContext(NotificationContext);
  const handleReload = useCallback(() => {
    setLoading(true);
    createViewSql(uuid, view, viewKind)
      .then((e) => {
        setSql(e);
      })
      .catch((e) => {
        showNotification("error", "fetch create sql failed", e);
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
          }}
        >
          reload
        </Button>
      </Stack>
      {sql && (
        <SimpleFlexEditor
          readOnly={true}
          language={getLanguageFromDataSourceKind(kind)}
          value={sql}
          handleEditor={(editor) => {
            SetAutoCompletionName.set(
              editor.getModel().id,
              new NameAndQueryId(uuid, kind),
            );
          }}
        />
      )}
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
        <Stack direction="row" justifyContent="center">
          <ButtonGroup>
            {structures.map((e) => {
              return (
                <Button
                  disabled={structure == e}
                  onClick={() => {
                    setStructure(e);
                  }}
                  variant="contained"
                  key={e}
                >
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
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
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
  const setting = useContext(SettingContext);

  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const handleReload = useCallback(() => {
    setLoading(true);
    showViewColumns(uuid, view)
      .then((fs) => {
        setFields(fs);
      })
      .catch((e) => {
        showNotification("error", "list column failed", e);
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
    const width = ColumnWidth.instance.get(e.field);
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
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

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
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
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
          }}
        >
          <Stack direction="row">
            {icon} <span> {view} </span>
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
  const { showNotification } = useContext(NotificationContext);
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
    let invokeName;
    switch (viewKind) {
      case ViewKind.View:
        invokeName = "drop_view";
        break;
      case ViewKind.MaterializedView:
        invokeName = "drop_materialized_view";
        break;
    }
    invoke(invokeName, { name: uuid, viewName: view })
      .then(() => {
        showNotification("success", "Drop View OK");
        reload();
      })
      .catch((e) => {
        showNotification("error", "Drop View Failure", e);
      });
  };

  const { tabs, setCurrentTab, appendTab } = useContext(TabsSetContext);

  const handleViewData = () => {
    const id = uuid + "-view-" + generateRandomString();

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
        showNotification("error", `data source ${kind} need impl view`);
    }

    appendTab(
      new TabContent(id, view, list, <VisibilityIcon className="tab-icon-size" />),
    );
  };

  const handleStructure = () => {
    const id = uuid + "-view-" + view + "-structure";
    const index = tabsHasId(tabs, id);
    if (typeof index == "number") {
      setCurrentTab(index);
    } else {
      appendTab(
        new TabContent(
          id,
          `${view}`,
          <ViewStructure view={view} uuid={uuid} kind={kind} viewKind={viewKind} />,
          <IoConstructOutline className="tab-icon-size" />,
        ),
      );
    }
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
            setShowExport(true);
          }}
        >
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
          }}
        >
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
          })}
        >
          <Box sx={modalStyle} width="45%">
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
