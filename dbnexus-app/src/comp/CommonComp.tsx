import {
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
  Box,
  Paper,
  MenuList,
} from '@mui/material';
import { ReactNode, useContext, useEffect, useMemo, useState } from 'react';
import { types } from './ApiStruct';
import { DataGrid, GridSlotsComponentsProps } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import RefreshIcon from '@mui/icons-material/Refresh';
import AddIcon from '@mui/icons-material/Add';
import { BsDatabaseAdd } from 'react-icons/bs';
import LinkOffIcon from '@mui/icons-material/LinkOff';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import {
  appStore,
  AppStoreKey,
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  DataSourceKind,
  errToString,
  handleCopyToClipboard,
  secondConfirm,
} from './helper';
import { TbDatabasePlus } from 'react-icons/tb';
import LinkIcon from '@mui/icons-material/Link';

import CloseIcon from '@mui/icons-material/Close';

const LogLevelSel = ({ handleChange, level }) => {
  let levels = ['trace', 'debug', 'info', 'warn', 'error'];

  return (
    <FormControl style={{ width: '100px' }}>
      <InputLabel id="dropdown-label">log level</InputLabel>
      <Select
        labelId="dropdown-label"
        value={level}
        onChange={(e) => {
          handleChange(e.target.value);
        }}
        variant="outlined"
      >
        {levels.map((e) => (
          <MenuItem value={e} key={e}>
            {e}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};

const datagridRowHeight = 25;
const datagridHeaderHeight = 36;

function StickyNav(props: { children: ReactNode[] | ReactNode }) {
  const { children } = props;

  return (
    <div style={{ position: 'sticky', top: 0, border: '1px blue solid' }}>
      {children}
    </div>
  );
}

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
      <ListItemText> create database </ListItemText>
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
      <ListItemText> refresh </ListItemText>
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
      <ListItemText> close connection </ListItemText>
    </MenuItem>
  );
}

function ConnStatus({ uuid }) {
  const [status, setStatus] = useState('');
  const [err, setErr] = useState('');
  const reloadStatus = () => {
    invoke<string>('check_conn', { name: uuid })
      .then((ping) => {
        setStatus(ping);
      })
      .catch((e) => {
        setErr(errToString(e));
      });
  };

  useEffect(() => {
    reloadStatus();
    let r = setInterval(() => {
      reloadStatus();
    }, 5000);
    return () => {
      clearInterval(r);
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
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}
    >
      <ListItemIcon>
        <TbDatabasePlus></TbDatabasePlus>
      </ListItemIcon>
      <ListItemText>
        create
        {kind == DataSourceKind.MsSql || kind == DataSourceKind.Postgres
          ? ' schema'
          : ' database'}
      </ListItemText>
    </MenuItem>
  );
}

function MenuOpenConn({ handleOpend, name }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={() => {
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
      <ListItemText> open connection </ListItemText>
    </MenuItem>
  );
}
const pageSizeOptions = [10, 20, 50, 100];

function SelectPageSize({
  width = '200px',
  variant = 'outlined',
  tital = 'select page size',
  pageSize,
  handleChange,
  options = pageSizeOptions,
}) {
  return (
    <Box width={width}>
      <FormControl
        //@ts-ignore
        variant={variant}
        fullWidth
      >
        <InputLabel id="simple-select-label">{tital}</InputLabel>
        <Select
          labelId="simple-select-label"
          id="simple-select"
          value={pageSize}
          onChange={(e) => {
            handleChange(e.target.value);
          }}
          label="select page size"
        >
          {options.map((e) => (
            <MenuItem key={e} value={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
    </Box>
  );
}

function MenuDeleteConfig({ name }) {
  const { showNotification } = useContext(NotificationContext);
  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: `drop connection '${name}'`,
      confirmText: 'drop',
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
      let old = await (
        await appStore()
      ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
      let newdata = old.filter((e) => e.name != name);
      await (await appStore()).set(AppStoreKey.ConnectionConfig, newdata);
      await invoke('close_conn', { name });
    } catch (e) {
      showNotification('error', 'drop connection failed', e);
    }
  };

  return (
    <MenuItem
      onClick={() => {
        handleDelete();
      }}
    >
      <ListItemIcon>
        <CloseIcon></CloseIcon>
      </ListItemIcon>
      <ListItemText> drop connection </ListItemText>
    </MenuItem>
  );
}
function MetaValuesView({ values = [] as types.MetaValue[], flex = 1 }) {
  let columns = [
    { field: 'name', flex },
    { field: 'value', flex },
    { field: 'desc', flex },
  ];

  values.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidv4();
    }
  });

  function Menu(props: { content: string }) {
    const { content } = props;

    const { showNotification } = useContext(NotificationContext);
    return (
      <Paper>
        <MenuList>
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> copy </ListItemText>
          </MenuItem>
        </MenuList>
      </Paper>
    );
  }
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          let content = (target as HTMLDivElement).innerHTML;

          setUpMenu({
            content: <Menu content={content}></Menu>,
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  return (
    <DataGrid
      slotProps={slotProps}
      rowHeight={datagridRowHeight}
      columnHeaderHeight={datagridHeaderHeight}
      columns={columns}
      rows={values}
    ></DataGrid>
  );
}

function MenuResetColumnWidths() {
  const { showNotification } = useContext(NotificationContext);
  const handleReset = async () => {
    columnWidths.reset();
    showNotification('success', 'reset column width success');
  };
  return (
    <MenuItem onClick={handleReset}>
      <ListItemIcon>
        <RefreshIcon></RefreshIcon>
      </ListItemIcon>
      <ListItemText> reset column width </ListItemText>
    </MenuItem>
  );
}

export {
  LogLevelSel,
  StickyNav,
  MetaValuesView,
  datagridRowHeight,
  datagridHeaderHeight,
  MenuNewQuery,
  MenuCreateDatabase,
  MenuRefresh,
  ConnStatus,
  MenuCloseConn,
  MenuCreateSchema,
  MenuOpenConn,
  SelectPageSize,
  pageSizeOptions,
  MenuDeleteConfig,
  MenuResetColumnWidths,
};
