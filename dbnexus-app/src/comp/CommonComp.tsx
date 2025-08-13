import {
  MenuItem,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
  MenuList,
  Stack,
  Divider,
  Chip,
} from '@mui/material';
import {
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { types } from './ApiStruct';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  DataGrid,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import CheckIcon from '@mui/icons-material/Check';
import CloseIcon from '@mui/icons-material/Close';
import { v4 as uuidv4 } from 'uuid';
import RefreshIcon from '@mui/icons-material/Refresh';
import AddIcon from '@mui/icons-material/Add';
import { BsDatabaseAdd } from 'react-icons/bs';
import LinkOffIcon from '@mui/icons-material/LinkOff';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import {
  appStore,
  AppStoreKey,
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  DataSourceKind,
  errToString,
  FavoriteSql,
  handleCopyToClipboard,
  saveAndSetSize,
  secondConfirm,
} from './helper';
import { TbDatabasePlus } from 'react-icons/tb';
import LinkIcon from '@mui/icons-material/Link';
import { getNamedConfig, SettingContext } from './Common';

const LogLevelSelection = ({ handleChange, level }) => {
  let levels = ['trace', 'debug', 'info', 'warn', 'error'];

  const getColor = (level: string) => {
    switch (level) {
      case 'trace':
        return 'primary';
      case 'debug':
        return 'secondary';
      case 'info':
        return 'info';
      case 'warn':
        return 'warning';
      case 'error':
        return 'error';
    }
  };

  return (
    <Stack direction="row" spacing={1}>
      {levels.map((e) => {
        return (
          <Chip
            key={e}
            size="small"
            disabled={level == e}
            color={getColor(e)}
            onClick={() => {
              handleChange(e);
            }}
            label={e}
          ></Chip>
        );
      })}
    </Stack>
  );
};

function MenuNewQuery({ handle }) {
  return (
    <MenuItem
      onClick={() => {
        handle();
      }}
    >
      <ListItemIcon>
        <AddIcon></AddIcon>
      </ListItemIcon>
      <ListItemText> New Query </ListItemText>
    </MenuItem>
  );
}
function MenuCreateDatabase({ handleCreate }) {
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}
    >
      <ListItemIcon>
        <BsDatabaseAdd></BsDatabaseAdd>
      </ListItemIcon>
      <ListItemText> Create Database </ListItemText>
    </MenuItem>
  );
}
function MenuRefresh({ reload }) {
  return (
    <MenuItem
      onClick={() => {
        reload();
      }}
    >
      <ListItemIcon>
        <RefreshIcon></RefreshIcon>
      </ListItemIcon>
      <ListItemText> Refresh </ListItemText>
    </MenuItem>
  );
}

function MenuCloseConn({ handleClose, name }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={() => {
        invoke('close_conn', { name })
          .then(() => {
            handleClose();
          })
          .catch((e) => {
            showNotification('error', 'open connection failed', e);
          });
      }}
    >
      <ListItemIcon>
        <LinkOffIcon></LinkOffIcon>
      </ListItemIcon>
      <ListItemText> Close Connection </ListItemText>
    </MenuItem>
  );
}

function ConnPingTime({ uuid }) {
  const [status, setStatus] = useState('');
  const [err, setErr] = useState('');
  const reloadStatus = () => {
    invoke<string>('check_conn', { name: uuid })
      .then((ping) => {
        setStatus(ping);
        setErr('');
      })
      .catch((e) => {
        setErr(errToString(e));
        setStatus('');
      });
  };

  useEffect(() => {
    reloadStatus();
    let interval = setInterval(() => {
      reloadStatus();
    }, 5000);
    return () => {
      clearInterval(interval);
    };
  }, []);

  return (
    (status || err) &&
    (status ? (
      <Tooltip title="ping time">
        <Typography color="green"> {status} </Typography>
      </Tooltip>
    ) : (
      <Tooltip title={err}>
        <Typography color="error"> err </Typography>
      </Tooltip>
    ))
  );
}

function MenuCreateSchema(props: { handleCreate; kind: DataSourceKind }) {
  const { handleCreate, kind } = props;
  if (kind === DataSourceKind.Sqlite) {
    return (
      <MenuItem
        onClick={() => {
          handleCreate();
        }}
      >
        <ListItemIcon>
          <TbDatabasePlus></TbDatabasePlus>
        </ListItemIcon>
        <ListItemText> attach database </ListItemText>
      </MenuItem>
    );
  }
  let name = 'database';
  switch (kind) {
    case DataSourceKind.MsSql:
    case DataSourceKind.Postgres:
    case DataSourceKind.DuckDB:
    case DataSourceKind.Cockroach:
      name = 'Schema';
      break;
    case DataSourceKind.Cassandra:
      name = 'Key Space';
      break;
  }
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}
    >
      <ListItemIcon>
        <TbDatabasePlus></TbDatabasePlus>
      </ListItemIcon>
      <ListItemText> {`Create ${name}`} </ListItemText>
    </MenuItem>
  );
}

function MenuOpenConn({ handleOpend, name }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={async () => {
        // try promote use to enter a password.
        let config = await getNamedConfig(name);
        if (!config) {
          return;
        }
        const has = await invoke('has_entered_password', { name: name });
        if (!has) {
          let key = 'save_password';
          if (false === config[key]) {
            let pass = prompt(`enter password for '${name}'`);
            if (pass == null) {
              return;
            }
            try {
              await invoke('save_entered_password', { name: name, pass: pass });
            } catch (e) {
              showNotification('error', 'password not correct???', e);
              return;
            }
          }
        }
        invoke('open_conn', { name })
          .then(() => {
            handleOpend();
          })
          .catch((e) => {
            showNotification('error', 'open connection failed', e);
          });
      }}
    >
      <ListItemIcon>
        <LinkIcon></LinkIcon>
      </ListItemIcon>
      <ListItemText> Open Connection </ListItemText>
    </MenuItem>
  );
}
const pageSizeOptions = [10, 20, 50, 100];

function SelectPageSize({ pageSize, handleChange, options = pageSizeOptions }) {
  return (
    <Stack direction="row" spacing={1}>
      {options.map((e) => (
        <Chip
          label={e}
          color="info"
          onClick={() => {
            handleChange(e);
          }}
          key={e}
          disabled={e == pageSize}
        ></Chip>
      ))}
    </Stack>
  );
}

function MenuDeleteConfig({ name }) {
  const { showNotification } = useContext(NotificationContext);
  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: `drop connection '${name}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    try {
      await invoke('delete_password', { name });
    } catch (e) {
      console.log('delete password err:', e);
    }
    try {
      let old = await appStore.get<types.DBConnectionConfig[]>(
        AppStoreKey.ConnectionConfig
      );
      let newdata = old.filter((e) => e.name != name);
      await appStore.set(AppStoreKey.ConnectionConfig, newdata);
      await invoke('close_conn', { name });
      await FavoriteSql.deleteByUuid(name);
    } catch (e) {
      showNotification('error', 'drop connection failed', e);
    }
  };
  return (
    <MenuItem
      onClick={() => {
        handleDelete();
      }}
      sx={{ color: 'error.main' }}
    >
      <ListItemIcon>
        <DeleteForeverIcon color="error"></DeleteForeverIcon>
      </ListItemIcon>
      <ListItemText color="error">Drop Connection</ListItemText>
    </MenuItem>
  );
}
function MetaValuesView({ values = [] as types.MetaValue[], flex = 1 }) {
  let columns = [
    { field: 'name', flex },
    { field: 'value', flex },
    { field: 'desc', flex },
  ];
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  values.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidv4();
    }
  });

  function Menu(props: { content: string; setSort; field: string }) {
    const { content, setSort, field } = props;

    const { showNotification } = useContext(NotificationContext);
    return (
      <MenuList>
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
        <Divider></Divider>
        <MenuItem
          onClick={() => {
            setSort([{ field, sort: 'asc' }] as GridSortModel);
          }}
        >
          <ListItemIcon>
            <ArrowUpwardIcon></ArrowUpwardIcon>
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setSort([{ field, sort: 'desc' }] as GridSortModel);
          }}
        >
          <ListItemIcon>
            <ArrowDownwardIcon></ArrowDownwardIcon>
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>
      </MenuList>
    );
  }
  const { setUpMenu } = useContext(ContextMenuContext);
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
            content: (
              <Menu
                content={content}
                field={field}
                setSort={setSortModel}
              ></Menu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);
  const setting = useContext(SettingContext);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);

  return (
    <div
      style={{
        display: 'grid',
      }}
    >
      <DataGrid
        style={gridStyle}
        sortModel={sortModel}
        onSortModelChange={onSortModelChange}
        showCellVerticalBorder
        showColumnVerticalBorder
        slotProps={slotProps}
        rowHeight={setting.datagridRowHeight}
        columnHeaderHeight={setting.datagridHeaderHeight}
        columns={columns}
        rows={values}
      ></DataGrid>
    </div>
  );
}

function MenuResetColumnWidths(props: { reset: () => void }) {
  const { showNotification } = useContext(NotificationContext);
  const { reset } = props;

  const handleReset = async () => {
    columnWidths.reset();
    reset();
    showNotification('success', 'reset column width successfully');
  };
  return (
    <MenuItem onClick={handleReset}>
      <ListItemIcon>
        <RefreshIcon></RefreshIcon>
      </ListItemIcon>
      <ListItemText> Reset Column Width </ListItemText>
    </MenuItem>
  );
}

function TrueOrFalse(props: { b: boolean }) {
  const { b } = props;
  return b ? <CheckIcon> </CheckIcon> : <CloseIcon></CloseIcon>;
}

function ModalHeader(props: { title: string }) {
  const { title } = props;
  return (
    <div>
      <Typography
        variant="h6"
        style={{
          textTransform: 'capitalize',
          paddingBottom: '0.3rem',
          fontWeight: 'bold',
        }}
      >
        {title}
      </Typography>
    </div>
  );
}

function ResizeWrapper(props: {
  children;
  sizeKey: string;
  sizeKeyKind: 'both' | 'width' | 'height';
  style: CSSProperties;
}) {
  const { children, sizeKey, sizeKeyKind, style } = props;
  const [dom, setDom] = useState(null);

  useEffect(() => {
    if (dom) {
      const c = saveAndSetSize(dom, sizeKey, sizeKeyKind);
      return c;
    }
  }, [dom]);
  return (
    <div
      style={{
        overflow: 'auto',
        display: 'flex',
        flexDirection: 'column',
        ...style,
      }}
      ref={setDom}
    >
      {children}
    </div>
  );
}

export {
  LogLevelSelection,
  MetaValuesView,
  MenuNewQuery,
  MenuCreateDatabase,
  MenuRefresh,
  ConnPingTime,
  MenuCloseConn,
  MenuCreateSchema,
  MenuOpenConn,
  SelectPageSize,
  pageSizeOptions,
  MenuDeleteConfig,
  MenuResetColumnWidths,
  TrueOrFalse,
  ModalHeader,
  ResizeWrapper,
};
