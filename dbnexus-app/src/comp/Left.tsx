'use strict';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  FormControl,
  IconButton,
  InputAdornment,
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Select,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import ClearIcon from '@mui/icons-material/Clear';
import {
  memo,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { GrConnect } from 'react-icons/gr';
import {
  AppStoreKey,
  OpenedContext,
  modalStyle,
  dropDatabase,
  dropSchema,
  QueryConn,
  EscDownEvent,
  modalHandleClose,
} from './Common';
import { LuDatabase } from 'react-icons/lu';
import { FaDatabase } from 'react-icons/fa';
import { FaCopy } from 'react-icons/fa';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { SimpleTreeView } from '@mui/x-tree-view/SimpleTreeView';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import CloseIcon from '@mui/icons-material/Close';
import logoImg from '../../icon.png';

import { TabContent } from './Tabs';
import wcmatch from 'wildcard-match';

import {
  appStore,
  ColumnWidth,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  dataGridParentStyle,
  DataSourceKind,
  extractConfigNameFromUuid,
  extractFieldAndIDFromMouseEvent,
  FavoriteSql,
  generateRandomString,
  handleCopyToClipboard,
  makeGetParserErr,
  MatchPatternContext,
  SaveLeftTopShow,
  SaveMatchPattern,
  secondConfirm,
  SettingContext,
  SomeTempStore,
  TableType,
  TabsSetContext,
  UuidContext,
  WindowFocusContext,
} from './helper';
import { TbDatabaseSearch } from 'react-icons/tb';
import { SqlQuery } from './SqlQuery';
import { RedisQuery } from './RedisQuery';
import { MongoQuery } from './MongoQuery.tsx';
import React from 'react';
import {
  ConnPingTime,
  MenuCloseConn,
  MenuCreateDatabase,
  MenuCreateSchema,
  MenuDeleteConfig,
  MenuNewQuery,
  MenuNewQueryWindow,
  MenuOpenConn,
  MenuRefresh,
  ModalHeader,
} from './CommonComp.tsx';
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import { RedisTables, TablesTree } from './LeftTable.tsx';
import { ViewKind, ViewsTree } from './LeftView.tsx';
import { CassandraAggregatesTree, UdfsTree } from './LeftUdf.tsx';
import { ProceduresTree } from './LeftProcedure.tsx';
import {
  DataGrid,
  GridColDef,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import { listen } from '@tauri-apps/api/event';
import { FaBookmark } from 'react-icons/fa';
import { SequenceTree } from './LeftSequence.tsx';
import { openUrl } from '@tauri-apps/plugin-opener';
import { TypesTree } from './LeftType.tsx';
import {
  CreateDatabase,
  CreateSchema,
  MenuConnectionInfo,
} from './CommonComp2.tsx';
import { EventsTree } from './LeftEvent.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

const addConnectionID = generateRandomString();
function LeftTop(props: { matchPattern: string[]; setMatchPattern }) {
  const { matchPattern, setMatchPattern } = props;
  const [matchPatterInput, setMatchPatternInput] = useState('');
  const [showFavoriteSql, setShowFavoriteSql] = useState(false);
  const [showCopyConn, setShowCopyConn] = useState(false);
  const [show, setShow] = useState(SaveLeftTopShow.instance.get());
  useEffect(() => {
    SaveLeftTopShow.instance.save(show);
  }, [show]);

  const showAddConnection = async (conn?: types.DBConnectionConfig) => {
    const id = addConnectionID + '-' + (conn ? conn.name : '');
    const webview = await WebviewWindow.getByLabel(id);
    if (webview) {
      await webview.setFocus();
    } else {
      SomeTempStore.set(conn);
      const webview = new WebviewWindow(id, {
        title: conn ? `copy connection of '${conn.name}'` : 'add connection',
        url: 'add_connection.html',
        width: 800,
        height: 600,
        maximizable: true,
      });
      webview.once('tauri://created', () =>
        console.log('create add connection view successfully')
      );
      webview.once('tauri://error', (e) => {
        showNotification('error', 'create webview failed', e.event.toString());
      });
    }
  };

  useEffect(() => {
    const unlisten = listen('add_config', () => {
      showAddConnection();
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, []);
  const { showNotification } = useContext(NotificationContext);
  const toggleShow = () => {
    setShow((e) => !e);
  };
  const theme = useTheme();
  return (
    <Stack spacing={1} style={{ padding: '2px' }}>
      <Stack>
        {show ? (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowDownIcon></KeyboardArrowDownIcon>
          </IconButton>
        ) : (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowRightIcon></KeyboardArrowRightIcon>
          </IconButton>
        )}
      </Stack>

      <Stack
        direction="row"
        style={{
          justifyContent: 'center',
          flexWrap: 'wrap',
          display: show ? 'flex' : 'none',
        }}
      >
        <Tooltip title="DBPal site">
          <img
            src={logoImg}
            style={{ width: '3.3rem', cursor: 'pointer' }}
            onClick={() => {
              openUrl('https://dbpal.site/');
            }}
            alt="logo"
          ></img>
        </Tooltip>

        <Tooltip title="make a connection">
          <IconButton
            onClick={() => {
              showAddConnection();
            }}
          >
            <GrConnect style={{ fontSize: '2.1rem' }}> </GrConnect>
          </IconButton>
        </Tooltip>

        <Tooltip title="Favorite sql statement">
          <IconButton
            onClick={() => {
              setShowFavoriteSql(true);
            }}
          >
            <FaBookmark style={{ fontSize: '2.1rem' }}> </FaBookmark>
          </IconButton>
        </Tooltip>

        <Tooltip title="copy a connection">
          <IconButton
            onClick={() => {
              setShowCopyConn(true);
            }}
          >
            <FaCopy style={{ fontSize: '2.2rem' }}> </FaCopy>
          </IconButton>
        </Tooltip>
      </Stack>

      <Stack
        spacing={1}
        style={{
          display: show ? 'flex' : 'none',
          border: '2px solid ' + theme.palette.success.main,
          borderRadius: '0.5rem',
          padding: '2px 0.5rem',
        }}
      >
        <Typography color="info">
          match pattern for table,view,udf,procedure etc
        </Typography>
        <Stack
          direction="row"
          style={{ flexWrap: 'wrap', alignItems: 'center' }}
          spacing={1}
        >
          {matchPattern.length > 1 && (
            <IconButton
              onClick={() => {
                setMatchPattern([]);
              }}
            >
              <CloseIcon></CloseIcon>
            </IconButton>
          )}
          {matchPattern.map((e, index) => (
            <Chip
              key={index}
              label={e}
              color="success"
              onDelete={() => {
                setMatchPattern((arr) =>
                  arr.filter((_, eIndex) => eIndex !== index)
                );
              }}
            />
          ))}
        </Stack>
        <Stack direction="row" spacing={1} style={{ alignItems: 'flex-start' }}>
          <TextField
            fullWidth
            value={matchPatterInput}
            label="match pattern"
            onChange={(e) => {
              setMatchPatternInput(e.target.value.trim());
            }}
            slotProps={{
              input: {
                endAdornment: matchPatterInput && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setMatchPatternInput('');
                      }}
                      edge="end"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          ></TextField>
          <Button
            onClick={() => {
              if (matchPattern.find((e) => e == matchPatterInput)) {
                showNotification('error', 'the value already exists');
                return;
              }
              if (!matchPatterInput) {
                showNotification('error', 'please input match pattern');
                return;
              }
              setMatchPattern((e) => [...e, matchPatterInput]);
            }}
            variant="outlined"
          >
            apply
          </Button>
        </Stack>
      </Stack>

      {showFavoriteSql && (
        <Modal
          open={showFavoriteSql}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFavoriteSql(false);
          })}
        >
          <Box sx={modalStyle} width="75%" height="65vh">
            <ModalHeader
              title="Favorite SQL Statements"
              handleClose={() => {
                setShowFavoriteSql(false);
              }}
            ></ModalHeader>
            <FavoriteSqlList
              handleClose={() => {
                setShowFavoriteSql(false);
              }}
            ></FavoriteSqlList>
          </Box>
        </Modal>
      )}
      {showCopyConn && (
        <Modal
          open={showCopyConn}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowCopyConn(false);
          })}
        >
          <Box sx={modalStyle} width="65%" height="60vh">
            <ModalHeader
              handleClose={() => {
                setShowCopyConn(false);
              }}
              title="Copy Connection"
            ></ModalHeader>
            <CopyConnection
              handleClose={() => {
                setShowCopyConn(false);
              }}
              setCopyConn={(conn) => {
                showAddConnection(conn);
              }}
            ></CopyConnection>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function CopyConnection(props: { handleClose; setCopyConn }) {
  const { handleClose, setCopyConn } = props;
  const [conns, setConns] = useState<types.DBConnectionConfig[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const getRowId = useCallback((e) => {
    return e.name;
  }, []);
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string }) {
    const { content } = props;
    return (
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}
      </MenuList>
    );
  }

  const useRow = async (conn) => {
    const password = await invoke<string>('get_password', { name: conn.name });
    const x = { ...conn };
    x['password'] = password;
    setCopyConn(x);
    handleClose();
  };
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const field = extractFieldAndIDFromMouseEvent(e).field;
          if (!field) {
            return;
          }
          setUpMenu({
            content: <Menu content={content}></Menu>,
            event: e,
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const cols = useMemo(() => {
    const cols = [
      { field: 'name' },
      {
        field: 'conn_type',
        renderHeader: () => {
          return 'kind';
        },
      },
      { field: 'host' },
      { field: 'port' },
      { field: 'path' },
      {
        field: 'db',
        renderHeader: () => {
          return 'database';
        },
      },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = ColumnWidth.instance.get(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleReload = async () => {
    try {
      const conns = await appStore.get<types.DBConnectionConfig[]>(
        AppStoreKey.ConnectionConfig
      );
      setConns(conns);
    } catch (e) {
      showNotification('error', 'get connections failed', e);
    }
  };
  useEffect(() => {
    handleReload();
  }, []);

  return (
    <Stack spacing={1} style={{ height: '100%' }}>
      <Stack direction="row">
        {conns.length == 0 && (
          <Button
            onClick={() => {
              handleReload();
            }}
          >
            reload
          </Button>
        )}
        <Tooltip title="double click a row to copy a connection.">
          <Typography color="info">copy</Typography>
        </Tooltip>
      </Stack>

      <div style={{ ...dataGridParentStyle }}>
        <DataGrid
          showToolbar
          getRowId={getRowId}
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.appearance.datagridRowHeight()}
          columnHeaderHeight={setting.appearance.datagridRowHeight()}
          columns={cols}
          rows={conns}
          onColumnResize={onColumnResize}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>
    </Stack>
  );
}

function Left_() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [matchPattern, setMatchPattern] = useState<string[]>(
    SaveMatchPattern.instance.get()
  );

  useEffect(() => {
    SaveMatchPattern.instance.save(matchPattern);
  }, [matchPattern]);

  const handleReload = async () => {
    appStore
      .get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig)
      .then((e) => {
        if (!e) {
          e = [];
        }
        setConfigs(e);
      })
      .catch((e) => {
        showNotification('error', 'list all config failure', e);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  appStore.onKeyChange(AppStoreKey.ConnectionConfig, () => {
    handleReload();
  });
  const matchFN = useCallback(
    (x: string) => {
      if (matchPattern.length == 0) {
        return true;
      }
      for (const mp of matchPattern) {
        const matchFn = wcmatch('*' + mp + '*', { flags: 'i' });
        if (matchFn(x)) {
          return true;
        }
      }
      return false;
    },
    [matchPattern]
  );
  const windowFocus = useContext(WindowFocusContext);
  return (
    <Stack
      style={{
        overflow: 'auto',
        opacity: windowFocus ? 1 : '0.6',
      }}
    >
      <LeftTop
        matchPattern={matchPattern}
        setMatchPattern={setMatchPattern}
      ></LeftTop>
      <Stack spacing={0.3}>
        {configs.map((e) => {
          return (
            <MatchPatternContext.Provider value={matchFN} key={e.name}>
              <ConnTypeContext.Provider
                value={{ kind: connTypeToDataSourceKind(e.conn_type) }}
              >
                <UuidContext.Provider
                  value={{
                    uuid: e.name,
                  }}
                >
                  <OneConnection key={e.name} config={e}></OneConnection>
                </UuidContext.Provider>
              </ConnTypeContext.Provider>
            </MatchPatternContext.Provider>
          );
        })}
      </Stack>
    </Stack>
  );
}

const makeConn = async (uuid: string, kind: DataSourceKind) => {
  const queryId = await invoke<number>('query_sql_open_connection', {
    name: uuid,
  });
  console.log('open connection:', queryId);
  return new QueryConn(uuid, queryId, kind);
};

function FavoriteSqlMenu(props: {
  content: string;
  id: number;
  handleReload;
  openInNewWindow: () => void;
}) {
  const { content, id, handleReload, openInNewWindow } = props;
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
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem onClick={openInNewWindow}>
        <ListItemIcon>
          <DeleteForeverIcon></DeleteForeverIcon>
        </ListItemIcon>
        <ListItemText> open query in new window </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={async () => {
          await FavoriteSql.delete(id, showNotification);
          handleReload(false);
        }}
      >
        <ListItemIcon>
          <DeleteForeverIcon></DeleteForeverIcon>
        </ListItemIcon>
        <ListItemText> delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function FavoriteSqlList(props: { handleClose }) {
  const { handleClose } = props;
  const { showNotification } = useContext(NotificationContext);
  const [fs, setFs] = useState<FavoriteSql[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: 'lastModified', sort: 'desc' },
  ]);
  const [uuidFilter, setUuidFilter] = useState('');
  const [distinctUuid, setDistinctUuid] = useState<string[]>([]);
  const setting = useContext(SettingContext);
  const handleReload = async (showOK) => {
    try {
      const us = await FavoriteSql.getDistinctUuid();
      setDistinctUuid(us);
      const fs = await FavoriteSql.getByUuid(uuidFilter);
      setFs(fs);
    } catch (e) {
      showNotification('error', 'list all favorite sql failed', e);
      return;
    }
    if (showOK) {
      showNotification('success', 'fetch favorite sql ok');
    }
  };

  const { setTabs, setSelectTab } = useContext(TabsSetContext);

  const checkConnOpened = async (uuid: string) => {
    const connOpened = await invoke<boolean>('conn_opened', { name: uuid });
    if (!connOpened) {
      showNotification(
        'error',
        `connection '${extractConfigNameFromUuid(uuid)}' not opened????`
      );
      return false;
    } else {
      return true;
    }
  };
  const useRow = async (row: FavoriteSql) => {
    try {
      if (!(await checkConnOpened(row.uuid))) {
        return;
      }
      setTabs((tabs) => {
        const content = (
          <AllKindQuery
            kind={row.kind}
            uuid={row.uuid}
            value={row.sql}
            favoriteName={row.name}
          />
        );
        handleClose();
        setTimeout(() => {
          setSelectTab(tabs.length);
        }, 100);
        return [
          ...tabs,
          new TabContent(
            generateRandomString(10),
            `${row.uuid}`,
            content,
            <TbDatabaseSearch> </TbDatabaseSearch>
          ),
        ];
      });
    } catch (e) {
      showNotification('error', 'use sql failure', e);
    }
  };
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  useEffect(() => {
    handleReload(false);
  }, [uuidFilter]);
  const { setUpMenu } = useContext(ContextMenuContext);
  const openInNewWindow = async (row: FavoriteSql) => {
    if (!(await checkConnOpened(row.uuid))) {
      return;
    }
    SomeTempStore.set(row);
    const webview = new WebviewWindow(generateRandomString(), {
      title: `query window of '${row.uuid}'`,
      url: 'query.html',
      width: 800,
      height: 600,
      maximized: true,
    });
    webview.once('tauri://created', () =>
      console.log('create query view successfully')
    );
    webview.once('tauri://error', (e) => {
      showNotification('error', 'create webview failed', e.event.toString());
    });
    return;
  };

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = fs.find((e) => e.id === parseInt(id));
          setUpMenu({
            content: (
              <FavoriteSqlMenu
                content={content}
                id={parseInt(id)}
                handleReload={handleReload}
                openInNewWindow={() => {
                  openInNewWindow(row);
                }}
              ></FavoriteSqlMenu>
            ),
            event: e,
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [fs]);

  const columns = useMemo(() => {
    const cols = [
      {
        field: 'uuid',
        renderHeader: () => {
          return 'connection name';
        },
      },
      { field: 'name' },
      { field: 'kind' },
      { field: 'sql' },
      { field: 'lastModified' },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = ColumnWidth.instance.get(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);
  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: 'clean all favorite sql???',
      confirmText: 'Clean',
      kind: 'error',
    });
    if (!confirm) {
      return;
    }
    FavoriteSql.clearByUuid(showNotification, uuidFilter);
  };

  return (
    <Stack spacing={1} style={{ height: '100%' }}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <FormControl style={{ width: '15rem' }}>
          <InputLabel> connection name </InputLabel>
          <Select
            value={uuidFilter}
            onChange={(e) => setUuidFilter(e.target.value)}
          >
            <MenuItem value=""></MenuItem>
            {distinctUuid.map((e) => (
              <MenuItem value={e} key={e}>
                {e}
              </MenuItem>
            ))}
          </Select>
        </FormControl>

        <Button
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
        {fs.length > 0 && uuidFilter && (
          <Tooltip title={`delete all sql by uuid '${uuidFilter}'`}>
            <IconButton
              onClick={() => {
                handleDelete();
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        )}
        {fs.length > 0 && (
          <>
            <Tooltip title="double click a row to use sql in new tab.">
              <Typography color="info">use</Typography>
            </Tooltip>
            <Tooltip title="right click a row to query in new window.">
              <Typography color="info">query new window</Typography>
            </Tooltip>
          </>
        )}
      </Stack>
      <div style={{ ...dataGridParentStyle }}>
        <DataGrid
          showToolbar
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.appearance.datagridRowHeight()}
          columnHeaderHeight={setting.appearance.datagridRowHeight()}
          columns={columns}
          rows={fs}
          onColumnResize={onColumnResize}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>
    </Stack>
  );
}

const OneConnection = memo(OneConfig_);
function OneConfig_(props: { config: types.DBConnectionConfig }) {
  const { config } = props;
  const [opened, setOpened] = useState(false);

  const makeTree = () => {
    switch (config.conn_type) {
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.TiDB:
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.Sqlite:
      case DataSourceKind.LibSql:
        return <SchemaTree></SchemaTree>;
      case DataSourceKind.Redis:
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.Mongo:
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.SqlServer:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Oracle:
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Cassandra:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.ClickHouse:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.FireBird:
        return <SchemaTree />;
      case DataSourceKind.DuckDB:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      default:
        console.log('impl database kind:', config);
        console.log(`impl this kind ${config.conn_type}`);
    }
  };
  return (
    <OpenedContext.Provider value={{ opened, setOpened }}>
      <SimpleTreeView disableSelection>{makeTree()}</SimpleTreeView>
    </OpenedContext.Provider>
  );
}

function SchemaTree(props: { schema?: string; reloadSchemas? }) {
  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const { schema, reloadSchemas } = props;

  const { opened, setOpened } = useContext(OpenedContext);
  const SUPPORTED_PROCEDURE_KINDS = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.SqlServer,
    DataSourceKind.Oracle,
    DataSourceKind.Cockroach,
  ]);

  const supportProcedure = SUPPORTED_PROCEDURE_KINDS.has(kind);
  const supportUdf = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.SqlServer,
    DataSourceKind.Oracle,
    DataSourceKind.Cassandra,
    DataSourceKind.Cockroach,
    DataSourceKind.ClickHouse,
  ]).has(kind);
  const supportSequence = [
    DataSourceKind.MariaDB,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
  ].includes(kind);

  const supportType = [
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Cassandra,
  ].includes(kind);
  const supportEvent = [DataSourceKind.Mysql, DataSourceKind.MariaDB].includes(
    kind
  );

  const { setUpMenu } = useContext(ContextMenuContext);
  const [loading, setLoading] = useState(false);
  const idForTree = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTree.current}
      label=<Stack
        spacing={1}
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <SchemaMenu
                opened={opened}
                schema={schema}
                setLoading={setLoading}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                }}
                uuid={uuid}
                reloadSchemas={reloadSchemas}
                kind={kind}
              ></SchemaMenu>
            ),

            event: e,
          });
        }}
        sx={{
          alignItems: 'center',
        }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnPingTime uuid={uuid}></ConnPingTime>}
        {loading && <CircularProgress />}
      </Stack>
    >
      {opened && kind == DataSourceKind.Sqlite && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.DuckDB && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.FireBird && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.LibSql && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.Cassandra && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized}></ViewsTree>
        </>
      )}
      {opened && kind == DataSourceKind.Mongo && (
        <>
          <TablesTree typ={TableType.Normal} />
          <TablesTree typ={TableType.MongoTimeSerial} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened &&
        (kind == DataSourceKind.Mysql ||
          kind == DataSourceKind.MariaDB ||
          kind == DataSourceKind.TiDB) && (
          <>
            <TablesTree typ={TableType.Normal} />
            <ViewsTree viewKind={ViewKind.Normal} />
          </>
        )}
      {opened && kind == DataSourceKind.ClickHouse && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Postgres && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Cockroach && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Oracle && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.SqlServer && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && (
        <>
          {supportProcedure && <ProceduresTree />}
          {supportUdf && <UdfsTree></UdfsTree>}
        </>
      )}
      {opened && <>{supportSequence && <SequenceTree />}</>}

      {opened && kind == DataSourceKind.Cassandra && (
        <CassandraAggregatesTree />
      )}
      {opened && kind == DataSourceKind.Redis && <RedisTables />}
      {opened && supportType && <TypesTree />}
      {opened && supportEvent && <EventsTree />}
    </TreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  uuid: string;
  reload;
  handleOpened;
  opened;
  handleCloseConn;
  reloadDatabase;
  kind: DataSourceKind;
  schemas: string[];
  setLoading;
}) {
  const {
    database,
    uuid,
    reload,
    handleOpened,
    opened,
    handleCloseConn,
    reloadDatabase,
    kind,
    schemas,
    setLoading,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showCreateSchema, setShowCreateSchema] = useState(false);
  const { setSelectTab, setTabs } = useContext(TabsSetContext);
  return (
    <div>
      <MenuList>
        {opened && kind != DataSourceKind.Mongo && (
          <MenuNewQuery
            handleNew={() => {
              openNewQuery(
                setSelectTab,
                setTabs,
                uuid,
                kind,
                showNotification,
                false
              );
            }}
          ></MenuNewQuery>
        )}
        {opened && kind != DataSourceKind.Mongo && (
          <MenuNewQueryWindow
            handleNew={() => {
              openNewQuery(
                setSelectTab,
                setTabs,
                uuid,
                kind,
                showNotification,
                true
              );
            }}
          ></MenuNewQueryWindow>
        )}

        {opened && kind != DataSourceKind.Oracle && (
          <MenuCreateSchema
            handleCreate={() => {
              setShowCreateSchema(true);
            }}
            kind={kind}
          ></MenuCreateSchema>
        )}

        {!opened && !database && (
          <MenuOpenConn
            name={uuid}
            setLoading={setLoading}
            handleOpened={handleOpened}
          ></MenuOpenConn>
        )}

        {opened && <MenuRefresh reload={reload}></MenuRefresh>}
        {opened && !database && (
          <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>
        )}

        {opened && !database && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}
        {database && kind != DataSourceKind.DuckDB && (
          <MenuItem
            onClick={() => {
              // we are in database context uuid contains the database name
              // we need remove that
              const index = uuid.indexOf('/');
              dropDatabase({
                uuid: uuid.substring(0, index),
                database,
                showNotification,
                okCallback: () => {
                  reloadDatabase(true);
                },
              });
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText color="error">drop</ListItemText>
          </MenuItem>
        )}

        {!database && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
      </MenuList>

      {showCreateSchema && (
        <Modal
          open={showCreateSchema}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowCreateSchema(false);
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader
              title={(function () {
                let name = 'database';
                switch (kind) {
                  case DataSourceKind.Postgres:
                  case DataSourceKind.Cockroach:
                  case DataSourceKind.SqlServer:
                    name = 'schema';
                    break;
                }
                return `create ${name}`;
              })()}
              handleClose={() => {
                setShowCreateSchema(false);
              }}
            ></ModalHeader>
            <CreateSchema
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateSchema(false);
              }}
              kind={kind}
              schemas={schemas}
              handleReload={reload}
            ></CreateSchema>
          </Box>
        </Modal>
      )}
    </div>
  );
}

function CatalogMenu(props: {
  uuid;
  reload;
  handleOpened;
  opened;
  handleCloseConn;
  setLoading;
  kind: DataSourceKind;
}) {
  const {
    uuid,
    reload,
    handleOpened,
    opened,
    handleCloseConn,
    kind,
    setLoading,
  } = props;
  const [showCreateDB, setShowCreateDB] = useState(false);
  return (
    <div>
      <MenuList>
        {!opened && (
          <MenuOpenConn
            name={uuid}
            setLoading={setLoading}
            handleOpened={handleOpened}
          ></MenuOpenConn>
        )}

        {opened && kind != DataSourceKind.DuckDB && (
          <MenuCreateDatabase
            handleCreate={() => {
              setShowCreateDB(true);
            }}
          ></MenuCreateDatabase>
        )}

        {opened && <MenuRefresh reload={reload}></MenuRefresh>}
        {opened && <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>}

        {opened && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}
        <Divider />
        <MenuDeleteConfig name={uuid}></MenuDeleteConfig>
      </MenuList>
      {showCreateDB && (
        <Modal
          open={showCreateDB}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowCreateDB(false);
          })}
        >
          <Box sx={modalStyle} width="50%">
            <ModalHeader
              handleClose={() => {
                setShowCreateDB(false);
              }}
              title="Create Database"
            ></ModalHeader>
            <CreateDatabase
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateDB(false);
              }}
            ></CreateDatabase>
          </Box>
        </Modal>
      )}
    </div>
  );
}

function AllKindQuery(props: {
  kind: DataSourceKind;
  uuid: string;
  value?: string;
  favoriteName?: string;
}) {
  const { kind, uuid, value, favoriteName } = props;
  let content: ReactNode;
  const { showNotification } = useContext(NotificationContext);
  switch (kind) {
    case DataSourceKind.SqlServer:
    case DataSourceKind.Sqlite:
    case DataSourceKind.Mysql:
    case DataSourceKind.MariaDB:
    case DataSourceKind.Postgres:
    case DataSourceKind.Oracle:
    case DataSourceKind.TiDB:
    case DataSourceKind.Cassandra:
    case DataSourceKind.ClickHouse:
    case DataSourceKind.LibSql:
    case DataSourceKind.FireBird:
    case DataSourceKind.DuckDB:
    case DataSourceKind.Cockroach:
      content = (
        <SqlQuery
          uuid={uuid}
          makeConn={async () => {
            return makeConn(uuid, kind);
          }}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
          value={value}
          getParseErr={makeGetParserErr(uuid)}
          kind={kind}
          favoriteName={favoriteName}
        ></SqlQuery>
      );
      break;
    case DataSourceKind.Redis:
      content = (
        <RedisQuery
          makeConn={async () => {
            return makeConn(uuid, kind);
          }}
          uuid={uuid}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
          value={value}
          getParseErr={makeGetParserErr(uuid)}
          favoriteName={favoriteName}
        ></RedisQuery>
      );
      break;

    case DataSourceKind.Mongo:
      content = (
        <MongoQuery
          uuid={uuid}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
          value={value}
          favoriteName={favoriteName}
        ></MongoQuery>
      );
      break;
    default:
      console.log(`impl this ${kind}`);
  }
  return <div style={{ height: '100%' }}> {content}</div>;
}

function openNewQuery(
  setSelectTab,
  setTabs,
  uuid: string,
  kind: DataSourceKind,
  showNotification,
  newWindow: boolean
) {
  if (newWindow) {
    SomeTempStore.set({ uuid, kind } as FavoriteSql);
    const webview = new WebviewWindow(generateRandomString(), {
      title: `query window of '${uuid}'`,
      url: 'query.html',
      width: 800,
      height: 600,
      maximized: true,
    });
    webview.once('tauri://created', () =>
      console.log('create query view successfully')
    );
    webview.once('tauri://error', (e) => {
      showNotification('error', 'create webview failed', e.event.toString());
    });
    return;
  }
  console.log('open new query uuid:', uuid);
  const id = uuid + '-query-' + generateRandomString();
  const content = <AllKindQuery kind={kind} uuid={uuid} />;
  setTabs((e: TabContent[]) => {
    setTimeout(() => {
      setSelectTab(e.length);
    }, 100);
    return [
      ...e,
      new TabContent(
        id,
        `${uuid}`,
        content,
        <TbDatabaseSearch> </TbDatabaseSearch>
      ),
    ];
  });
}
function SchemaMenu(props: {
  schema?: string;
  uuid: string;
  handleOpened;
  opened;
  handleCloseConn;
  kind: DataSourceKind;
  reloadSchemas?;
  setLoading;
}) {
  const {
    schema,
    uuid,
    handleOpened,
    opened,
    handleCloseConn,
    reloadSchemas,
    kind,
    setLoading,
  } = props;
  const { setSelectTab, setTabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuList>
      {!schema && !opened && (
        <MenuOpenConn
          name={uuid}
          setLoading={setLoading}
          handleOpened={handleOpened}
        ></MenuOpenConn>
      )}

      {opened && (
        <MenuNewQuery
          handleNew={() => {
            openNewQuery(
              setSelectTab,
              setTabs,
              uuid,
              kind,
              showNotification,
              false
            );
          }}
        ></MenuNewQuery>
      )}

      {opened && (
        <MenuNewQueryWindow
          handleNew={() => {
            openNewQuery(
              setSelectTab,
              setTabs,
              uuid,
              kind,
              showNotification,
              true
            );
          }}
        ></MenuNewQueryWindow>
      )}

      {opened && !schema && (
        <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>
      )}

      {!schema && opened && (
        <MenuCloseConn
          name={uuid}
          handleClose={handleCloseConn}
        ></MenuCloseConn>
      )}
      {schema && kind != DataSourceKind.Oracle && (
        <MenuItem
          onClick={() => {
            // we are in database context uuid contains the database name
            // we need remove that
            const index = uuid.lastIndexOf('/');
            dropSchema({
              uuid: uuid.substring(0, index),
              schema,
              showNotification,
              okCallback: () => {
                reloadSchemas(true);
              },
            });
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText color="error">drop</ListItemText>
        </MenuItem>
      )}
      {!schema && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
    </MenuList>
  );
}

function CatalogTree(_props: {}) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [databases, setDatabases] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const { kind } = useContext(ConnTypeContext);
  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_database', { name })
      .then((ds) => {
        setDatabases(ds);
      })
      .catch((e) => {
        showNotification('error', 'list database failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const { opened, setOpened } = useContext(OpenedContext);
  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);

  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        spacing={1}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <CatalogMenu
                uuid={name}
                setLoading={setLoading}
                reload={handleReload}
                opened={opened}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                  setDatabases([]);
                }}
                kind={kind}
              ></CatalogMenu>
            ),
            event: e,
          });
        }}
      >
        <span>{name} </span>
        {loading && <CircularProgress />}
        {opened && <ConnPingTime uuid={name}></ConnPingTime>}
      </Stack>
      key={name}
    >
      {databases.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + '/' + e }}>
            <DatabaseTree
              key={e}
              database={e}
              reloadDatabase={handleReload}
            ></DatabaseTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

function DatabaseTree(props: {
  database?: string;
  reloadDatabase?: () => void;
}) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [schemas, setSchemas] = useState<string[]>([]);
  const { database, reloadDatabase } = props;
  const [loading, setLoading] = useState(false);
  const { opened, setOpened } = useContext(OpenedContext);
  const { kind } = useContext(ConnTypeContext);
  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_schema', { name })
      .then((ds) => {
        setSchemas(ds);
      })
      .catch((e) => {
        showNotification('error', 'list schema failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);
  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        spacing={1}
        direction="row"
        alignContent="baseline"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <DatabaseMenu
                setLoading={setLoading}
                opened={opened}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                  setSchemas([]);
                }}
                database={database}
                uuid={name}
                reload={handleReload}
                reloadDatabase={reloadDatabase}
                kind={kind}
                schemas={schemas}
              ></DatabaseMenu>
            ),
            event: e,
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {loading && <CircularProgress />}
        {!database && opened && <ConnPingTime uuid={name}></ConnPingTime>}
      </Stack>
      key={name + database}
    >
      {schemas.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + '/' + e }}>
            <SchemaTree
              key={e}
              schema={e}
              reloadSchemas={handleReload}
            ></SchemaTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}
const Left = React.memo(Left_);

export { Left, AllKindQuery };
