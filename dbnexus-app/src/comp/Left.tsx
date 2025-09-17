import {
  Box,
  Button,
  CircularProgress,
  Divider,
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
  Typography,
} from '@mui/material';
import ClearIcon from '@mui/icons-material/Clear';
import {
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { GrConnect } from 'react-icons/gr';
import {
  progressBarSizeSmall,
  appStore,
  AppStoreKey,
  OpenedContext,
  modalStyle,
  dropDatabase,
  dropSchema,
  initMonacoLanguage,
  QueryConn,
  EscDownEvent,
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

import logoImg from '../../icon.png';

import { TabContent } from './Tabs';
import wcmatch from 'wildcard-match';

import {
  calMenuStyle,
  columnWidths,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  DataSourceKind,
  extractConfigNameFromUuid,
  FavoriteSql,
  generateRandomString,
  gridStyle,
  handleCopyToClipboard,
  makeGetParserErr,
  MatchPatternContext,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
  WindowFocusChangeContext,
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
  MenuOpenConn,
  MenuRefresh,
  ModalHeader,
} from './CommonComp.tsx';
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import { RedisTables, TablesTree, TablesType } from './LeftTable.tsx';
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
import { AddConnection } from './AddConnection.tsx';
import { FaBookmark } from 'react-icons/fa';
import { SequenceTree } from './LeftSequence.tsx';
import { openUrl } from '@tauri-apps/plugin-opener';
import { TypesTree } from './LeftTypes.tsx';
import {
  CreateDatabase,
  CreateSchema,
  MenuConnectionInfo,
} from './CommonComp2.tsx';

function LeftTop(props: { matchPattern: string; setMatchPattern }) {
  const { matchPattern, setMatchPattern } = props;
  const [showFavoriteSql, setShowFavoriteSql] = useState(false);
  const [showAddConnection, setShowAddConnection] = useState(false);
  const [copyConn, setCopyConn] = useState<types.DBConnectionConfig>(null);
  const [showCopyConn, setShowCopyConn] = useState(false);
  const [show, setShow] = useState(false);
  useEffect(() => {
    let unlisten = listen('add_config', () => {
      setShowAddConnection(true);
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  const toggleShow = () => {
    setShow((e) => !e);
  };

  return (
    <Stack spacing={1} style={{ padding: '4px 12px' }}>
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

      {show && (
        <Stack
          direction="row"
          style={{ justifyContent: 'center', flexWrap: 'wrap' }}
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
                setShowAddConnection(true);
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
      )}
      {show && (
        <TextField
          fullWidth
          value={matchPattern}
          size="small"
          label="match pattern for table view udf procedure ..."
          onChange={(e) => {
            setMatchPattern(e.target.value.trim());
          }}
          slotProps={{
            input: {
              endAdornment: matchPattern && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setMatchPattern('');
                    }}
                    edge="end"
                    size="small"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          helperText="name etc"
        ></TextField>
      )}

      {showFavoriteSql && (
        <Modal
          open={showFavoriteSql}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowFavoriteSql(false);
          }}
        >
          <Box sx={modalStyle} width="75%">
            <ModalHeader title="Favorite SQL Statements"></ModalHeader>
            <FavoriteSqlList
              handleClose={() => {
                setShowFavoriteSql(false);
              }}
            ></FavoriteSqlList>
          </Box>
        </Modal>
      )}

      {showAddConnection && (
        <Modal
          open={showAddConnection}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowAddConnection(false);
          }}
        >
          <Box sx={modalStyle} width="55%">
            <ModalHeader title="Add Connection"></ModalHeader>
            <AddConnection
              handleClose={() => {
                setShowAddConnection(false);
              }}
            ></AddConnection>
          </Box>
        </Modal>
      )}
      {copyConn && (
        <Modal
          open={!!copyConn}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setCopyConn(null);
          }}
        >
          <Box sx={modalStyle} width="55%">
            <ModalHeader title="Copy Connection"></ModalHeader>
            <AddConnection
              handleClose={() => {
                setCopyConn(null);
              }}
              config={copyConn}
            ></AddConnection>
          </Box>
        </Modal>
      )}

      {showCopyConn && (
        <Modal
          open={showCopyConn}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowCopyConn(false);
          }}
        >
          <Box sx={modalStyle} width="65%">
            <ModalHeader title="Copy Connection"></ModalHeader>
            <CopyConnection
              handleClose={() => {
                setShowCopyConn(false);
              }}
              setCopyConn={(conn) => {
                setCopyConn(conn);
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

  const useRow = (conn) => {
    setCopyConn(conn);
    handleClose();
  };
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: <Menu content={content}></Menu>,
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, []);

  let cols = useMemo(() => {
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
      const width = columnWidths.getWidth(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleReload = async () => {
    try {
      let conns = await appStore.get<types.DBConnectionConfig[]>(
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
    <Stack spacing={1}>
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

      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          style={gridStyle}
          getRowId={getRowId}
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
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
  const [matchPattern, setMatchPattern] = useState('');
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
  const m = useMemo(() => {
    return matchPattern
      ? wcmatch('*' + matchPattern + '*', { flags: 'i' })
      : () => {
          return true;
        };
  }, [matchPattern]);
  const windowFocus = useContext(WindowFocusChangeContext);
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
      <Divider />
      <Stack spacing={0.3}>
        {configs.map((e) => {
          return (
            <MatchPatternContext.Provider value={m} key={e.name}>
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
      <div style={{ minHeight: '300px' }}></div>
    </Stack>
  );
}

const makeConn = async (uuid: string, kind: DataSourceKind) => {
  const queryId = await invoke<string>('query_sql_open_connection', {
    name: uuid,
  });
  console.log('open connection:', queryId);
  return new QueryConn(uuid, queryId, kind);
};
function FavoriteSqlList(props: { handleClose }) {
  const { handleClose } = props;
  const { showNotification } = useContext(NotificationContext);
  const [sql, setSql] = useState<FavoriteSql[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: 'lastModified', sort: 'desc' },
  ]);
  const handleReload = async (showOK) => {
    try {
      const x = await FavoriteSql.getAll();
      setSql(x);
    } catch (e) {
      showNotification('error', 'list all favorite sql failed', e);
      return;
    }
    if (showOK) {
      showNotification('success', 'fetch favorite sql ok');
    }
  };
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const useRow = async (row: FavoriteSql) => {
    initMonacoLanguage(row.uuid, row.kind);
    try {
      let connOpened = await invoke<boolean>('conn_opened', { name: row.uuid });
      if (!connOpened) {
        showNotification(
          'error',
          `connection ${extractConfigNameFromUuid(row.uuid)} not opened????`
        );
        return;
      }

      setTabs((tabs) => {
        let content;
        switch (row.kind) {
          case DataSourceKind.Mongo:
            content = (
              <MongoQuery
                favoriteName={row.name}
                value={row.sql}
                uuid={row.uuid}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
              ></MongoQuery>
            );
            break;
          case DataSourceKind.Redis:
            content = (
              <RedisQuery
                language={row.uuid}
                makeConn={async () => {
                  return makeConn(row.uuid, row.kind);
                }}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
                value={row.sql}
                getParseErr={makeGetParserErr(row.uuid)}
                favoriteName={row.name}
              ></RedisQuery>
            );
            break;
          default:
            content = (
              <SqlQuery
                getParseErr={makeGetParserErr(row.uuid)}
                uuid={row.uuid}
                makeConn={async () => {
                  return makeConn(row.uuid, row.kind);
                }}
                value={row.sql}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
                favoriteName={row.name}
                kind={row.kind}
              ></SqlQuery>
            );
        }
        handleClose();
        setTimeout(() => {
          setSelectTab(tabs.length);
        }, 100);
        return [
          ...tabs,
          new TabContent(
            generateRandomString(10),
            `q@${row.uuid}`,
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
  }, []);
  const { setUpMenu } = useContext(ContextMenuContext);
  function Menu(props: { content: string; id: number }) {
    const { content, id } = props;
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
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target, currentTarget } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');

          setUpMenu({
            content: <Menu content={content} id={parseInt(id)}></Menu>,
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeCsvHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const setting = useContext(SettingContext);
  const columns = useMemo(() => {
    let cols = [
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
      let width = columnWidths.getWidth(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);
  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleClean = async () => {
    let confirm = await secondConfirm({
      title: 'clean all favorite sql???',
      confirmText: 'Clean',
    });
    if (!confirm) {
      return;
    }
    FavoriteSql.clear(showNotification);
    handleClose();
  };

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          variant="contained"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
        {sql.length > 0 && (
          <Tooltip title="delete all sql">
            <IconButton
              onClick={() => {
                handleClean();
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        )}
        {sql.length > 0 && (
          <Tooltip title="double click a row to use sql.">
            <Typography color="info">use</Typography>
          </Tooltip>
        )}
      </Stack>
      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          style={gridStyle}
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
          columns={columns}
          rows={sql}
          onColumnResize={onColumnResize}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>
    </Stack>
  );
}

const OneConnection = memo(OneConfig_);
function OneConfig_(props: { config: types.DBConnectionConfig }) {
  let { config } = props;
  const [opened, setOpened] = useState();

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
      case DataSourceKind.MsSql:
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
        throw `impl this kind`;
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
  const getPrefix = () => {
    return uuid + schema;
  };
  const { opened, setOpened } = useContext(OpenedContext);
  const SUPPORTED_PROCEDURE_KINDS = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.MsSql,
    DataSourceKind.Oracle,
    DataSourceKind.Cockroach,
  ]);

  let supportProcedure = SUPPORTED_PROCEDURE_KINDS.has(kind);
  let supportUdf = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.MsSql,
    DataSourceKind.Oracle,
    DataSourceKind.Cassandra,
    DataSourceKind.Cockroach,
    DataSourceKind.ClickHouse,
  ]).has(kind);
  let supportSequence = [
    DataSourceKind.MariaDB,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
  ].includes(kind);

  let supportType = [
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Cassandra,
  ].includes(kind);

  let { setUpMenu } = useContext(ContextMenuContext);
  const [loading, setLoading] = useState(false);
  return (
    <TreeItem
      itemId={getPrefix()}
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

            style: calMenuStyle(e),
          });
        }}
        sx={{
          alignItems: 'center',
        }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnPingTime uuid={uuid}></ConnPingTime>}
        {loading && <CircularProgress size={progressBarSizeSmall} />}
      </Stack>
    >
      {opened && kind == DataSourceKind.Sqlite && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.DuckDB && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.FireBird && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.LibSql && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.Cassandra && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized}></ViewsTree>
        </>
      )}
      {opened && kind == DataSourceKind.Mongo && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <TablesTree typ={TablesType.MongoTimeSerial} />
          <ViewsTree viewKind={ViewKind.Normal} />
        </>
      )}
      {opened &&
        (kind == DataSourceKind.Mysql ||
          kind == DataSourceKind.MariaDB ||
          kind == DataSourceKind.TiDB) && (
          <>
            <TablesTree typ={TablesType.Normal} />
            <ViewsTree viewKind={ViewKind.Normal} />
          </>
        )}
      {opened && kind == DataSourceKind.ClickHouse && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Postgres && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Cockroach && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Oracle && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewKind={ViewKind.Normal} />
          <ViewsTree viewKind={ViewKind.Materialized} />
        </>
      )}
      {opened && kind == DataSourceKind.MsSql && (
        <>
          <TablesTree typ={TablesType.Normal} />
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
      {opened && kind == DataSourceKind.Redis && (
        <RedisTables schema={schema} />
      )}
      {opened && supportType && <TypesTree />}
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
              openNewQuery(setSelectTab, setTabs, uuid, kind, showNotification);
            }}
          ></MenuNewQuery>
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
              let index = uuid.indexOf('/');
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
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowCreateSchema(false);
          }}
        >
          <Box sx={modalStyle} width="40%">
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
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowCreateDB(false);
          }}
        >
          <Box sx={modalStyle} width="50%">
            <ModalHeader title="Create Database"></ModalHeader>
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

function openNewQuery(
  setSelectTab,
  setTabs,
  uuid: string,
  kind: DataSourceKind,
  showNotification
) {
  initMonacoLanguage(uuid, kind);

  let id = uuid + '-query-' + generateRandomString();
  let content;

  switch (kind) {
    case DataSourceKind.MsSql:
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
          getParseErr={makeGetParserErr(uuid)}
          kind={kind}
        ></SqlQuery>
      );
      break;
    case DataSourceKind.Redis:
      content = (
        <RedisQuery
          language={uuid}
          makeConn={async () => {
            return makeConn(uuid, kind);
          }}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
          getParseErr={makeGetParserErr(uuid)}
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
        ></MongoQuery>
      );
      break;
    default:
      console.log(`impl this ${kind}`);
  }

  setTabs((e: TabContent[]) => {
    setTimeout(() => {
      setSelectTab(e.length);
    }, 100);

    return [
      ...e,
      new TabContent(
        id,
        `q@${uuid}`,
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
  const { showNotification } = useContext(NotificationContext);
  const { setSelectTab, setTabs } = useContext(TabsSetContext);
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
            openNewQuery(setSelectTab, setTabs, uuid, kind, showNotification);
          }}
        ></MenuNewQuery>
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
            let index = uuid.lastIndexOf('/');
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

  let { setUpMenu } = useContext(ContextMenuContext);

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
            style: calMenuStyle(e),
          });
        }}
      >
        <span>{name} </span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
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
  let { setUpMenu } = useContext(ContextMenuContext);

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
            style: calMenuStyle(e),
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
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

export { Left };
