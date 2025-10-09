'use strict';
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { NotificationContext } from './Notification';
import Stack from '@mui/material/Stack';
import InfoIcon from '@mui/icons-material/Info';
import CloseIcon from '@mui/icons-material/Close';
import SaveIcon from '@mui/icons-material/Save';
import {
  Button,
  Chip,
  FormControlLabel,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Radio,
  RadioGroup,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import {
  DataSourceKind,
  editorSetPositionAndReveal,
  generateRandomString,
  isMacOs,
  KeyPressed,
  secondConfirm,
  SettingContext,
  SomeTempStore,
  TabActiveContext,
} from './helper';
import { Editor, Monaco } from '@monaco-editor/react';
import { editor, Position } from 'monaco-editor';
import { invoke } from '@tauri-apps/api/core';
import { extractBaseName, getNamedConfig, reloadOK } from './Common';
import types from './ApiStruct';
import { MetaValueView } from './CommonComp';
import { readFile, writeFile } from '@tauri-apps/plugin-fs';
import { save, SaveDialogOptions, open } from '@tauri-apps/plugin-dialog';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

function CreateDatabase({ uuid, handleClose }) {
  const { showNotification } = useContext(NotificationContext);
  const [database, setDatabase] = useState('');
  const databaseDom = useRef(null);
  const handleCreate = () => {
    if (!database) {
      databaseDom.current.focus();
      showNotification('error', 'database name is empty');
      return;
    }

    let extra = null;
    invoke('create_database', { name: uuid, database, extra })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <Stack>
        <TextField
          inputRef={databaseDom}
          value={database}
          onChange={(e) => {
            setDatabase(e.target.value);
          }}
          label="database name"
        ></TextField>
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          create
        </Button>
      </Stack>
    </Stack>
  );
}

function CreateSchema(props: {
  uuid;
  handleClose;
  handleReload;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { uuid, handleClose, kind, schemas, handleReload } = props;
  const schemaSet = new Set(schemas);
  const { showNotification } = useContext(NotificationContext);
  const [schema, setSchema] = useState('');
  const schemaDom = useRef(null);
  const replicationStrategies = ['SimpleStrategy', 'NetworkTopologyStrategy'];
  const [replicationStrategy, setReplicationStrategy] = useState(
    replicationStrategies[0]
  );
  const [replicationFactor, setReplicationFactor] = useState('3');
  const handleCreate = () => {
    if (!schema) {
      schemaDom.current.focus();
      showNotification('error', 'Schema is empty');
      return;
    }
    if (schemaSet.has(schema)) {
      showNotification('error', `'${schema}' already exists.`);
      return;
    }
    let extra = null;
    switch (kind) {
      case DataSourceKind.Cassandra:
        extra = {
          extra: 'cassandra',
          replication_strategy: replicationStrategy,
          replication_factor: parseInt(replicationFactor),
        };
        break;
    }
    invoke('create_schema', { name: uuid, schema, extra })
      .then(() => {
        showNotification('success', 'Create OK');
        handleClose();
        handleReload();
      })
      .catch((e) => {
        showNotification('error', 'Create Failure', e);
      });
  };

  if (kind === DataSourceKind.Cassandra) {
    //
    return (
      <Stack spacing={1}>
        <RadioGroup
          row
          onChange={(e) => {
            setReplicationStrategy(e.target.value);
          }}
          value={replicationStrategy}
        >
          {replicationStrategies.map((e) => (
            <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
          ))}
        </RadioGroup>
        <TextField
          type="number"
          value={replicationFactor}
          onChange={(e) => {
            setReplicationFactor(e.target.value);
          }}
          label="replication factor"
        ></TextField>
        <TextField
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          inputRef={schemaDom}
          label="name"
        ></TextField>
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }
  if (kind === DataSourceKind.Redis) {
    // redis have special create
    const range = [];
    for (let i = 0; i < 16; i++) {
      range.push('db' + i);
    }
    return (
      <Stack spacing={1}>
        <Stack direction="row" style={{ flexWrap: 'wrap' }}>
          {range
            .filter((e) => !schemaSet.has(e))
            .map((e) => (
              <Chip
                key={e}
                onClick={() => {
                  setSchema(e);
                }}
                label={e}
                variant="outlined"
              ></Chip>
            ))}
        </Stack>
        <Stack>
          <TextField
            value={schema}
            onChange={(e) => {
              setSchema(e.target.value);
            }}
            inputRef={schemaDom}
            label="name"
          ></TextField>
        </Stack>
        <Stack direction="row">
          <Button
            onClick={() => {
              handleCreate();
            }}
          >
            create
          </Button>
        </Stack>
      </Stack>
    );
  }

  return (
    <Stack spacing={1}>
      <Stack>
        <TextField
          value={schema}
          onChange={(e) => {
            setSchema(e.target.value);
          }}
          label="name"
          inputRef={schemaDom}
        ></TextField>
      </Stack>
      <Stack direction="row">
        <Button
          onClick={() => {
            handleCreate();
          }}
        >
          create
        </Button>
      </Stack>
    </Stack>
  );
}

function SimpleFlexEditor(props: {
  language: string;
  value?: string;
  handleValue?: (x: string) => void;
  barLeft?: ReactNode;
  wordWrap?: string;
  readOnly?: boolean;
  errPos?: Position;
  handleEditor?: (editor: editor.IStandaloneCodeEditor) => void;
}) {
  let {
    language,
    value,
    handleValue,
    barLeft,
    wordWrap,
    readOnly,
    errPos,
    handleEditor,
  } = props;
  const [pos, setPos] = useState<Position>(null);
  const setting = useContext(SettingContext);
  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const handleDidMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, _monaco: Monaco) => {
      editor.current = ed;
      ed.focus();
      if (handleEditor) {
        handleEditor(ed);
      }
      ed.onDidChangeCursorPosition(() => {
        setPos(ed.getPosition());
      });
      ed.onDidChangeModelContent(() => {
        if (handleValue) {
          handleValue(ed.getValue());
        }
      });
    },
    []
  );
  useEffect(() => {
    if (editor.current && errPos) {
      editorSetPositionAndReveal(editor.current, errPos);
    }
  }, [errPos]);

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.fontSize,
      //@ts-ignore
      wordWrap: wordWrap,
      fontWeight: setting.fontWeight,
      fontFamily: setting.fontFamily,
      readOnly,
      tabSize: setting.tabSize,
      ...setting.editorOption,
    };
  }, [setting]);

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        flex: 1,
      }}
    >
      <div style={{ flex: 1 }}>
        <Editor
          width="99%"
          height="99%" // 不能设置成100%好像会影响父的高度
          onMount={handleDidMount}
          language={language}
          value={value}
          theme={setting.editorTheme()}
          options={options}
        ></Editor>
      </div>

      <EditorBar left={barLeft} pos={pos}></EditorBar>
    </div>
  );
}

function EditorBar(props: { left?: any; pos: Position }) {
  let { left, pos } = props;
  return (
    <Stack justifyContent="space-between" direction="row" alignItems="center">
      {left ? left : <span></span>}

      <Stack
        direction="row"
        spacing={1}
        style={{ paddingLeft: '2rem', paddingRight: '3rem' }}
      >
        <span>Ln {pos ? pos.lineNumber : ''}</span>
        <span>Col {pos ? pos.column : ''}</span>
      </Stack>
    </Stack>
  );
}

let infoWindows = new Map<String, WebviewWindow>();

function MenuConnectionInfo({ uuid }) {
  const { showNotification } = useContext(NotificationContext);
  const handleInfo = async () => {
    let old = infoWindows.get(uuid);
    if (old) {
      old.setFocus();
      return;
    }
    SomeTempStore.set(uuid);
    const webview = new WebviewWindow(generateRandomString(), {
      title: 'add connection',
      url: 'info.html',
      width: 800,
      height: 600,
    });
    webview.once('tauri://created', () =>
      console.log('create info view successfully')
    );
    webview.once('tauri://error', (e) => {
      showNotification('error', 'create webview failed', e.event.toString());
    });
    infoWindows.set(uuid, webview);
  };
  return (
    <MenuItem onClick={handleInfo}>
      <ListItemIcon>
        <InfoIcon></InfoIcon>
      </ListItemIcon>
      <ListItemText> Info </ListItemText>
    </MenuItem>
  );
}

function ConnConfigInfo(props: { uuid }) {
  let { uuid } = props;
  const { showNotification } = useContext(NotificationContext);
  const [metas, setMetas] = useState<types.MetaValue[]>([]);
  const handleInfo = async (showOK: boolean) => {
    try {
      let metas = await invoke<types.MetaValue[]>('server_metas', {
        name: uuid,
      });
      let config = await getNamedConfig(uuid);
      let configMetas = [];
      switch (config.conn_type) {
        case DataSourceKind.Mysql:
        case DataSourceKind.MariaDB:
        case DataSourceKind.TiDB:
        case DataSourceKind.Postgres:
        case DataSourceKind.Cockroach:
        case DataSourceKind.Redis:
        case DataSourceKind.Mongo:
        case DataSourceKind.Cassandra:
        case DataSourceKind.Oracle:
        case DataSourceKind.ClickHouse:
        case DataSourceKind.FireBird:
        case DataSourceKind.MsSql:
          configMetas = [
            { name: 'host', value: config.host } as types.MetaValue,
            { name: 'port', value: config.port + '' } as types.MetaValue,
            { name: 'user', value: config.username } as types.MetaValue,
          ];
          break;
        case DataSourceKind.LibSql:
          if (config.source === 'local') {
            configMetas = [
              { name: 'host', value: config.path } as types.MetaValue,
            ];
          } else if (config.source == 'remote') {
            configMetas = [
              { name: 'url', value: config.url } as types.MetaValue,
              { name: 'path', value: config.path } as types.MetaValue,
            ];
          }
          break;
        case DataSourceKind.Sqlite:
          configMetas = [
            { name: 'path', value: config.path } as types.MetaValue,
          ];
          break;
      }
      setMetas([...configMetas, ...metas]);
      if (showOK) {
        reloadOK(showNotification);
      }
    } catch (e) {
      showNotification('error', 'call server metas failed', e);
    }
  };
  useEffect(() => {
    handleInfo(false);
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Button
          color="primary"
          onClick={() => {
            handleInfo(true);
          }}
        >
          reload
        </Button>
      </Stack>

      <MetaValueView values={metas}></MetaValueView>
    </Stack>
  );
}

function LoadStoreFileForQuery(props: {
  getQuery: () => string;
  handleChange: (content) => void;
  kind: DataSourceKind;
}) {
  const { getQuery, handleChange, kind } = props;
  const [file, setFile] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const active = useContext(TabActiveContext);
  const handleLoad = async () => {
    // ok to load
    try {
      let file = await open();
      if (file) {
        const content = new TextDecoder().decode(await readFile(file));
        handleChange(content);
        setFile(file);
      }
    } catch (e) {
      showNotification('error', 'error to open file', e);
    }
  };
  const saveOpt = useMemo(() => {
    let filters;
    switch (kind) {
      case DataSourceKind.Mongo:
        filters = [
          {
            name: 'js',
            extensions: ['js'],
          },
        ];
        break;
      case DataSourceKind.Redis:
        filters = [
          {
            name: 'redis',
            extensions: ['redis'],
          },
        ];
        break;
      case DataSourceKind.Cassandra:
        filters = [
          {
            name: 'cql',
            extensions: ['cql'],
          },
        ];
        break;
      default:
        filters = [
          {
            name: 'SQL',
            extensions: ['sql'],
          },
        ];
    }

    return { title: 'save query', filters } as SaveDialogOptions;
  }, [kind]);

  const handleSave = async () => {
    let content = getQuery();
    if (!content) {
      showNotification('error', `can't save empty content`);
      return;
    }
    try {
      let ofile = file ? file : await save(saveOpt);
      if (ofile) {
        await writeFile(ofile, new TextEncoder().encode(content), {
          create: true,
        });
        setFile(ofile);
        handleChange(content);
        showNotification('success', 'save file OK');
      }
    } catch (e) {
      showNotification('error', 'save file failed', e);
    }
  };

  const handleClose = async () => {
    try {
      let fileContent = new TextDecoder().decode(await readFile(file));
      if (fileContent != getQuery()) {
        const confirm = await secondConfirm({
          title: 'file not saved',
          content: 'discard the content',
        });
        if (!confirm) {
          return;
        }
      }
      setFile('');
      handleChange('');
    } catch (e) {
      showNotification('error', 'close file failed', e);
    }
  };

  useEffect(() => {
    const callback = (e: KeyboardEvent) => {
      if (!active) {
        return;
      }

      if (KeyPressed.isControlSave(e)) {
        e.preventDefault();
        handleSave();
      }
      if (KeyPressed.isControlOpen(e)) {
        e.preventDefault();
        handleLoad();
      }
    };
    document.addEventListener('keydown', callback);
    return () => {
      document.removeEventListener('keydown', callback);
    };
  }, [active]);

  return (
    <>
      {file && (
        <Stack direction="row" style={{ alignItems: 'center' }}>
          <Tooltip title={file}>
            <Typography>{extractBaseName(file)}</Typography>
          </Tooltip>
          <Tooltip title="close file">
            <IconButton onClick={handleClose}>
              <CloseIcon></CloseIcon>
            </IconButton>
          </Tooltip>
        </Stack>
      )}
      <Tooltip
        title={`attach file,keyboard shortcut is ${
          isMacOs() ? 'command+o' : 'control+o'
        }`}
      >
        <IconButton onClick={handleLoad} color="primary">
          <AttachFileIcon />
        </IconButton>
      </Tooltip>
      <Tooltip
        title={`save query to file,keyboard shortcut is ${
          isMacOs() ? 'command+s' : 'control+s'
        }`}
      >
        <IconButton onClick={handleSave} color="primary">
          <SaveIcon />
        </IconButton>
      </Tooltip>
    </>
  );
}

export {
  CreateDatabase,
  CreateSchema,
  SimpleFlexEditor,
  EditorBar,
  MenuConnectionInfo,
  ConnConfigInfo,
  LoadStoreFileForQuery,
};
