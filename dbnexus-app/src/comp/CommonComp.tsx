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
  useTheme,
  Button,
  TextField,
  InputLabel,
  Select,
  FormControl,
  IconButton,
} from '@mui/material';
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { types } from './ApiStruct';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  DataGrid,
  GridColDef,
  GridColumnMenu,
  GridColumnMenuItemProps,
  GridColumnMenuProps,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import CheckIcon from '@mui/icons-material/Check';
import CloseIcon from '@mui/icons-material/Close';
import { v4 as uuidV4 } from 'uuid';
import RefreshIcon from '@mui/icons-material/Refresh';
import AddIcon from '@mui/icons-material/Add';
import { BsDatabaseAdd } from 'react-icons/bs';
import LinkOffIcon from '@mui/icons-material/LinkOff';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import { IoTimerOutline } from 'react-icons/io5';
import {
  appStore,
  AppStoreKey,
  calMenuStyle,
  ColumnWidth,
  ContextMenuContext,
  DataSourceKind,
  editorSetPositionAndReveal,
  ErrAndLoc,
  errToString,
  FavoriteSql,
  generateDefaultFilename,
  generateRandomString,
  gridStyle,
  handleCopyToClipboard,
  maxTreeSize,
  pageSizeOptions,
  QueryLogHandler,
  SaveAlign,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
  textEllipsisStyle,
} from './helper';

import { TbDatabasePlus } from 'react-icons/tb';
import LinkIcon from '@mui/icons-material/Link';
import { getNamedConfig } from './Common';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { editor, Position } from 'monaco-editor';
import { FaAlignLeft } from 'react-icons/fa';
import { FaAlignRight } from 'react-icons/fa';
import { FaAlignCenter } from 'react-icons/fa6';
import { openPath } from '@tauri-apps/plugin-opener';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { FaMarkdown } from 'react-icons/fa';

function MenuExportMarkDownTable({ showExportMarkDown }) {
  return (
    <MenuItem onClick={showExportMarkDown}>
      <ListItemIcon>
        <FaMarkdown fontSize="1.5rem" />
      </ListItemIcon>
      <ListItemText>Export MarkDown Table</ListItemText>
    </MenuItem>
  );
}
function SelectAlign(props: {
  align: 'left' | 'right' | 'center';
  handleChange;
}) {
  let theme = useTheme();
  const { align, handleChange } = props;
  const handleChangeXXX = (a) => {
    SaveAlign.instance.save(a);
    handleChange(a);
  };
  let style = { cursor: 'pointer' };

  return (
    <Stack direction="row" fontSize="1.5rem" spacing={1}>
      <FaAlignLeft
        color={align == 'left' && theme.palette.primary.main}
        onClick={() => {
          handleChangeXXX('left');
        }}
        style={style}
      />
      <FaAlignCenter
        color={align == 'center' && theme.palette.primary.main}
        onClick={() => {
          handleChangeXXX('center');
        }}
        style={style}
      />
      <FaAlignRight
        color={align == 'right' && theme.palette.primary.main}
        onClick={() => {
          handleChangeXXX('right');
        }}
        style={style}
      />
    </Stack>
  );
}

const LogLevelSelection = ({ handleChange, level }) => {
  let levels = ['trace', 'debug', 'info', 'warn', 'error'];

  return (
    <Stack style={{ minWidth: '13rem' }}>
      <FormControl fullWidth>
        <InputLabel>logger level</InputLabel>
        <Select
          value={level}
          label="logger level"
          onChange={(e) => handleChange(e.target.value)}
        >
          {levels.map((e) => {
            return (
              <MenuItem key={e} value={e}>
                {e}
              </MenuItem>
            );
          })}
        </Select>
      </FormControl>
    </Stack>
  );
};

function MenuNewQuery({ handleNew }) {
  return (
    <MenuItem
      onClick={() => {
        handleNew();
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
            showNotification('error', 'close connection failed', e);
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
  let name;
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
    default:
      name = 'database';
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

function MenuOpenConn({ handleOpened, name, setLoading }) {
  const { showNotification } = useContext(NotificationContext);

  const handleOpenXXX = async () => {
    // try promote use to enter a password.
    let config = await getNamedConfig(name);
    if (!config) {
      return;
    }
    const has = await invoke('has_entered_password', { name: name });
    if (!has) {
      let key = 'save_password';
      if (false === config[key]) {
        let pass = window.prompt(`enter password for '${name}'`);
        // macos will return null directly
        if (pass == null) {
          return;
        }
        try {
          await invoke('save_entered_password', {
            name: name,
            conn: config,
            pass: pass,
          });
        } catch (e) {
          showNotification('error', 'password not correct???', e);
          return;
        }
      }
    }
    try {
      await invoke('open_conn', { name });
    } catch (e) {
      showNotification('error', 'open connection failed', e);
      return;
    }
    handleOpened();
  };
  return (
    <MenuItem
      onClick={() => {
        setLoading(true);
        handleOpenXXX().finally(() => {
          setLoading(false);
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

function SelectPageSize({ pageSize, handleChange, options = pageSizeOptions }) {
  return (
    <Stack direction="row" spacing={1}>
      {options.map((e) => (
        <Chip
          label={e}
          onClick={() => {
            handleChange(e);
          }}
          key={e}
          color={e == pageSize ? 'primary' : 'default'}
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
      console.log(
        'delete password err:',
        e,
        "this should be fine maybe this connection don't have password"
      );
    }
    try {
      let old = await appStore.get<types.DBConnectionConfig[]>(
        AppStoreKey.ConnectionConfig
      );
      let newData = old.filter((e) => e.name != name);
      await invoke('close_conn', { name });
      await FavoriteSql.deleteByConnectionName(name);
      await QueryLogHandler.deleteByConnectionname(name);
      await appStore.set(AppStoreKey.ConnectionConfig, newData);
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
function MetaValuesView({ values = [] as types.MetaValue[] }) {
  const setting = useContext(SettingContext);
  let columns = [
    { field: 'name', width: ColumnWidth.instance.get('name') },
    { field: 'value', width: ColumnWidth.instance.get('value') },
    { field: 'desc', flex: 1 },
  ] as GridColDef[];

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  values.forEach((e) => {
    if (!('id' in e)) {
      //@ts-ignore
      e.id = uuidV4();
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
        <Divider />
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

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    let f = colDef.field;
    if (f == 'name' || f == 'value') {
      ColumnWidth.instance.save(f, width);
    }
  }, []);
  return (
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
        rows={values}
        onColumnResize={onColumnResize}
      ></DataGrid>
    </div>
  );
}

function MenuResetColumnWidths(props: { reset: () => void }) {
  const { showNotification } = useContext(NotificationContext);
  const { reset } = props;

  const handleReset = async () => {
    ColumnWidth.instance.reset();
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
          paddingBottom: '0.3rem',
          fontWeight: 'bold',
          textTransform: 'capitalize',
        }}
      >
        {title}
      </Typography>
    </div>
  );
}

function ResizeWrapper(props: {
  children: ReactNode;
  sizeKey: string;
  sizeKeyKind: 'both' | 'width' | 'height';
  style?: CSSProperties;
  height?: string | number;
  minHeight?: string | number;
}) {
  let { children, sizeKey, sizeKeyKind, style, height, minHeight } = props;
  if (!height) {
    height = '20vh';
  }
  if (!minHeight) {
    minHeight = '20vh';
  }
  const dom = useRef(null);
  useEffect(() => {
    const c = saveAndSetSize(dom.current, sizeKey, sizeKeyKind);
    return c;
  }, []);
  let resize;
  switch (sizeKeyKind) {
    case 'both':
      resize = 'both';
      break;
    case 'width':
      resize = 'horizontal';
      break;
    case 'height':
      resize = 'vertical';
      break;
  }
  return (
    <div
      style={{
        overflow: 'auto',
        display: 'flex',
        flexDirection: 'column',
        height,
        minHeight,
        resize,
        ...style,
      }}
      ref={dom}
    >
      {children}
    </div>
  );
}

function ViewRowInModal(props: { fields: types.Field[]; x: object; isTable? }) {
  const { fields, x, isTable } = props;
  const theme = useTheme();
  const setting = useContext(SettingContext);
  return (
    <Stack spacing={1}>
      {fields.map((f) => {
        return (
          <Stack
            style={{
              border: '2px solid',
              borderRadius: '10px',
              padding: '8px',
            }}
            key={f.field}
          >
            <Stack
              direction="row"
              spacing={1}
              style={{ alignItems: 'center' }}
              divider={<span>|</span>}
            >
              <Typography variant="subtitle1">{f.field}</Typography>
              <Typography variant="body1">{f.data_type}</Typography>
              {isTable && f.is_primary && (
                <Typography variant="body1"> Primary Key </Typography>
              )}
              {isTable && f.nullable && (
                <Typography variant="body1"> nullable </Typography>
              )}
              {isTable && f.is_auto_increment && (
                <Typography variant="body1"> auto increment </Typography>
              )}
              {isTable && f.default_value && (
                <Typography variant="body1" style={textEllipsisStyle}>
                  default value({f.default_value})
                </Typography>
              )}
            </Stack>

            <Divider style={{ borderWidth: '2px' }} />
            <pre
              style={{
                overflow: 'auto',
                opacity: 0.9,
                color: theme.palette.success.main,
                fontFamily: setting.fontFamily,
                fontWeight: setting.fontWeight,
              }}
            >
              {x[f.field]}
            </pre>
          </Stack>
        );
      })}
    </Stack>
  );
}

function ExceedMaxTreeItem(props: { exceed: number }) {
  const { exceed } = props;
  let id = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={id.current}
      style={{ display: exceed > maxTreeSize ? 'block' : 'none' }}
      label=<Typography color="error" style={{ fontSize: '0.8rem' }}>
        ({exceed}) exceed max tree items({maxTreeSize}),some item not showing
      </Typography>
    ></TreeItem>
  );
}

function UdfNameAndParameters(props: {
  udf: types.NamedStaff;
  kind: DataSourceKind;
}) {
  const { udf, kind } = props;
  let parameter = '';
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      parameter = udf.parameters;
      break;
    case DataSourceKind.Cassandra:
      parameter = udf.cassandra_parameters.join(',');
      break;
  }

  return (
    <>
      <Typography>{udf.name}</Typography>
      {parameter && (
        <Typography
          color="success"
          style={{
            fontStyle: 'italic',
            fontSize: '0.8rem',
            ...textEllipsisStyle,
          }}
        >
          ({parameter})
        </Typography>
      )}
    </>
  );
}

function PossibleParseError(props: {
  parseErr: ErrAndLoc;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { parseErr, editor } = props;
  return (
    <Tooltip title="possible error">
      <Typography
        style={{
          cursor: parseErr.line && parseErr.column && 'pointer',
          ...textEllipsisStyle,
        }}
        onClick={() => {
          if (!parseErr) {
            return;
          }
          if (parseErr.line && parseErr.column) {
            editorSetPositionAndReveal(
              editor,
              new Position(parseErr.line, parseErr.column)
            );
          }
        }}
      >
        {parseErr.msg}
      </Typography>
    </Tooltip>
  );
}

function ExecutingTime(props: { loading: boolean; disableTime?: boolean }) {
  const { loading, disableTime } = props;
  const executeTimeRef = useRef(null);
  const timeExecuteClean = useRef<() => void>(null);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());
  const tryExecuteTimeClean = () => {
    if (timeExecuteClean.current) {
      timeExecuteClean.current();
      timeExecuteClean.current = null;
    }
  };

  useEffect(() => {
    return () => {
      tryExecuteTimeClean();
    };
  }, []);
  const timeExecute = () => {
    const now = new Date();
    const setUp = () => {
      //@ts-ignore
      const ms = new Date() - now;
      function getTime() {
        let s = Math.floor(ms / 1000);
        if (s == 0) {
          return `${ms}ms`;
        }
        let r = ms % 1000;
        if (r == 0) {
          return `${s}s`;
        }
        return `${s}s ${r}ms`;
      }
      (executeTimeRef.current as HTMLDivElement).innerHTML = `${getTime()}`;
    };
    const int = setInterval(() => {
      setUp();
    }, 100);
    timeExecuteClean.current = () => {
      clearInterval(int);
      setUp();
    };
  };

  useEffect(() => {
    tryExecuteTimeClean();
    if (!loading) {
      setLastLoadTime(new Date());
    } else {
      timeExecute();
    }
  }, [loading]);

  return (
    <>
      <Tooltip title="execute time">
        <Chip
          color="success"
          label={<span ref={executeTimeRef}> </span>}
          style={{ minWidth: '6rem' }}
          icon={<IoTimerOutline fontSize="1.2rem" />}
        ></Chip>
      </Tooltip>
      {!disableTime && (
        <Typography>loaded at:{lastLoadTime.toLocaleString()}</Typography>
      )}
    </>
  );
}
function CopyMenuItem(props: GridColumnMenuItemProps) {
  const { colDef } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuItem
      onClick={() => {
        handleCopyToClipboard(colDef.field, showNotification);
      }}
    >
      <ListItemIcon> </ListItemIcon>
      <ListItemText> Copy Field </ListItemText>
    </MenuItem>
  );
}

function AppColumnMenu(props: GridColumnMenuProps) {
  return (
    <GridColumnMenu
      {...props}
      slots={{
        // Add new item
        closeColumnMenuCopyField: CopyMenuItem,
      }}
      slotProps={{
        closeColumnMenuCopyField: {
          displayOrder: 15,
        },
      }}
    />
  );
}

function SelectColumns(props: {
  fields: types.Field[];
  includes: string[];
  setIncludes;
}) {
  const { fields, includes, setIncludes } = props;
  const [matchStr, setMatchStr] = useState('');
  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
        spacing={1}
      >
        <Typography color="info">includes:</Typography>
        {includes.length > 0 && (
          <IconButton
            onClick={() => {
              setIncludes([]);
            }}
            color="error"
          >
            <CloseIcon></CloseIcon>
          </IconButton>
        )}
        {includes.map((x) => (
          <Chip
            onClick={() => {
              setIncludes((e) => e.filter((e) => e != x));
            }}
            key={x}
            color="success"
            label={x}
          ></Chip>
        ))}
      </Stack>
      <Stack direction="row" spacing={1} style={{ alignItems: 'flex-start' }}>
        <Button
          onClick={() => {
            setIncludes(
              fields
                .filter((e) => e.field.includes(matchStr))
                .map((x) => x.field)
            );
          }}
          disabled={includes.length == fields.length}
          variant="outlined"
        >
          include all
        </Button>
        <TextField
          value={matchStr}
          onChange={(e) => {
            setMatchStr(e.target.value);
          }}
          disabled={includes.length == fields.length}
          style={{ flex: 1 }}
          label="match field name string"
          helperText="only for field candidate"
        ></TextField>
      </Stack>
      <Stack
        direction="row"
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
        spacing={1}
      >
        <Typography>candidates:</Typography>
        {fields
          .filter((f) => includes.find((e) => e == f.field) == undefined)
          .filter((f) => f.field.includes(matchStr))
          .map((x) => (
            <Chip
              key={x.field}
              onClick={() => {
                setIncludes((e) => [...e, x.field]);
              }}
              variant="outlined"
              color="default"
              label={x.field}
            ></Chip>
          ))}
      </Stack>
    </Stack>
  );
}

function OpenFileWithDefaultApp({ file }) {
  const { showNotification } = useContext(NotificationContext);
  return (
    <>
      {file && (
        <Button
          onClick={() => {
            openPath(file)
              .then(() => {})
              .catch((e) => {
                showNotification('error', 'open export file failed', e);
              });
          }}
        >
          open with default app (if applicable)
        </Button>
      )}
    </>
  );
}

function ExportMarkDownTable(props: {
  fields: types.Field[];
  data: any[];
  name?: string;
}) {
  let { fields, data, name } = props;
  if (!name) {
    name = 'export';
  }
  const [includes, setIncludes] = useState<string[]>([]);
  const [align, setAlign] = useState('center');
  const { showNotification } = useContext(NotificationContext);
  const [file, setFile] = useState('');
  const makeMarkDownStr = () => {
    let ret = '';
    includes.forEach((x) => {
      ret += `| ${x} `;
    });
    ret += '|\n';
    includes.forEach(() => {
      let a;
      switch (align) {
        case 'left':
          a = ':----';
          break;
        case 'right':
          a = '-----:';
          break;
        case 'center':
          a = ':---:';
          break;
      }
      ret += '|' + a;
    });
    ret += '|\n';

    data.forEach((data) => {
      includes.forEach((x) => {
        ret += `| ${data[x]} `;
      });
      ret += '|\n';
    });
    return ret;
  };
  const handleExport = async (toClipBoard: boolean) => {
    if (includes.length == 0) {
      showNotification('error', 'Please select column');
      return;
    }
    let markdown = makeMarkDownStr();
    if (toClipBoard) {
      handleCopyToClipboard(markdown, undefined, () => {
        showNotification('success', 'export OK');
      });
      return;
    }
    const f = await save({
      defaultPath: generateDefaultFilename(name, 'md'),
      filters: [{ name: 'markdown', extensions: ['md'] }],
    });
    if (!f) {
      return;
    }
    try {
      await writeTextFile(f, markdown);
    } catch (e) {
      showNotification('error', 'write markdown failed', e);
      return;
    }
    showNotification('success', 'export OK');
    setFile(f);
  };

  return (
    <Stack spacing={1}>
      <SelectColumns
        fields={fields}
        includes={includes}
        setIncludes={setIncludes}
      ></SelectColumns>
      <OpenFileWithDefaultApp file={file} />
      <Divider></Divider>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          onClick={() => {
            handleExport(false);
          }}
          variant="outlined"
        >
          save file
        </Button>
        <Button
          onClick={() => {
            handleExport(true);
          }}
          variant="outlined"
        >
          copy to clipboard
        </Button>
        <SelectAlign
          align={SaveAlign.instance.get() as 'center'}
          handleChange={(e) => {
            setAlign(e);
          }}
        ></SelectAlign>
      </Stack>
    </Stack>
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
  ViewRowInModal,
  ExceedMaxTreeItem,
  UdfNameAndParameters,
  PossibleParseError,
  ExecutingTime,
  AppColumnMenu,
  ExportMarkDownTable,
  SelectColumns,
  OpenFileWithDefaultApp,
  MenuExportMarkDownTable,
};
