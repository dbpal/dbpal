'use strict';
import { useContext, useEffect, useRef, useState } from 'react';
import ClearIcon from '@mui/icons-material/Clear';
import {
  TextField,
  Button,
  Switch,
  FormControlLabel,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Stack,
  Divider,
  RadioGroup,
  Radio,
  Checkbox,
  Chip,
  Tooltip,
  InputAdornment,
  IconButton,
} from '@mui/material';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import VisibilityIcon from '@mui/icons-material/Visibility';
import { NotificationContext } from './Notification';
import { types } from './ApiStruct';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { AppStoreKey, extractBaseName, msgTimeout } from './Common';
import { appStore, DataSourceKind, errToString } from './helper';
import { openUrl } from '@tauri-apps/plugin-opener';
import { platform } from '@tauri-apps/plugin-os';

async function selectOneFile() {
  const selected = await open({
    multiple: false,
  });
  return selected;
}

function HostAndPort(props: { host; port; setHost; setPort; onBlur }) {
  let { host, port, setHost, setPort, onBlur } = props;
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="Host"
        value={host}
        onBlur={onBlur}
        sx={{ mr: 1 }}
        onChange={(e) => setHost(e.target.value)}
        slotProps={{
          input: {
            endAdornment: host && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setHost('');
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
      <TextField
        fullWidth
        label="port"
        onBlur={onBlur}
        error={isNaN(parseInt(port))}
        value={port}
        onChange={(e) => setPort(e.target.value)}
      />
    </Stack>
  );
}

function SelectPath(props: {
  path;
  setPath;
  label: string;
  required: boolean;
}) {
  let { path, setPath, label, required } = props;
  return (
    <Stack direction="row" sx={{ alignItems: 'center' }}>
      <TextField
        value={path}
        onChange={(e) => {
          setPath(e.target.value);
        }}
        style={{ flex: 1 }}
        label={label}
        required={required}
        slotProps={{
          input: {
            endAdornment: path && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setPath('');
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
      <IconButton
        onClick={() => {
          selectOneFile().then((e) => {
            setPath(e);
          });
        }}
        color="primary"
      >
        <AttachFileIcon></AttachFileIcon>
      </IconButton>
    </Stack>
  );
}

function UserAndPassword(props: {
  user;
  password;
  setUser;
  setPassword;
  onBlur;
  savePassword;
  setSavePassword;
}) {
  let {
    user,
    password,
    setUser,
    setPassword,
    onBlur,
    savePassword,
    setSavePassword,
  } = props;
  const [passwordVisible, setPasswordVisible] = useState(false);
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onBlur={onBlur}
        onChange={(e) => setUser(e.target.value)}
        slotProps={{
          input: {
            endAdornment: user && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setUser('');
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
      <TextField
        fullWidth
        label="Password"
        onBlur={onBlur}
        type={passwordVisible ? 'text' : 'password'}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        slotProps={{
          input: {
            endAdornment: !passwordVisible &&
              password != '' &&
              platform() != 'windows' && ( // window 已经有了这个功能了
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setPasswordVisible(true);
                    }}
                    edge="end"
                  >
                    <VisibilityIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
          },
        }}
      />
      <Stack
        style={{
          width: '31rem',
          justifyContent: 'center',
          alignItems: 'center',
        }}
        direction="row"
      >
        <Checkbox
          checked={savePassword}
          onChange={(e) => {
            setSavePassword(e.target.checked);
          }}
          name="save password"
        />
        save password
      </Stack>
    </Stack>
  );
}

//@ts-ignore
function EnableSSL(props: { useSSL; setUseSSL }) {
  let { useSSL, setUseSSL } = props;
  return (
    <FormControlLabel
      control={
        <Switch
          checked={useSSL}
          onChange={(e) => setUseSSL(e.target.checked)}
        />
      }
      label="Enable SSL"
    />
  );
}

function Https(props: { https; setHttps }) {
  let { https, setHttps } = props;
  return (
    <FormControlLabel
      control={
        <Switch checked={https} onChange={(e) => setHttps(e.target.checked)} />
      }
      label="HTTPS"
    />
  );
}

function TrustCert(props: { trustCert; setTrustCert }) {
  let { trustCert, setTrustCert } = props;
  return (
    <Stack direction="row" style={{ alignItems: 'center' }}>
      <FormControlLabel
        control={
          <Switch
            checked={trustCert}
            onChange={(e) => setTrustCert(e.target.checked)}
          />
        }
        label="Trust Cert"
      />
      <Chip
        label="?"
        onClick={() => {
          openUrl(
            'https://docs.rs/tiberius/0.12.3/tiberius/struct.Config.html#method.trust_cert'
          );
        }}
      ></Chip>
    </Stack>
  );
}

function EnableSSlCert(props: {
  sslCa;
  setSslCa;
  sslCert;
  setSslCert;
  enableSsl;
  setEnableSsl;
  sslKey;
  setSslKey;
}) {
  const {
    sslCa,
    setSslCa,
    sslCert,
    setSslCert,
    sslKey,
    setSslKey,
    enableSsl,
    setEnableSsl,
  } = props;

  return (
    <Stack
      direction="row"
      spacing={1}
      style={{ alignItems: 'center', flexWrap: 'wrap' }}
    >
      <>
        <Checkbox
          checked={enableSsl}
          onChange={(e) => {
            setEnableSsl(e.target.checked);
          }}
        ></Checkbox>
        enable ssl
      </>
      {enableSsl && (
        <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
          <Tooltip
            title={sslCa}
            onClick={async () => {
              let file = await selectOneFile();
              if (file) {
                setSslCa(file);
              }
            }}
          >
            <Chip
              label={
                'CA Cert(optional):' + (sslCa ? extractBaseName(sslCa) : '')
              }
              variant="outlined"
            ></Chip>
          </Tooltip>
          <Tooltip title={sslCert}>
            <Chip
              variant="outlined"
              onClick={async () => {
                let file = await selectOneFile();
                if (file) {
                  setSslCert(file);
                }
              }}
              label={
                'Certificate(optional):' +
                (sslCert ? extractBaseName(sslCert) : '')
              }
            ></Chip>
          </Tooltip>
          <Tooltip title={sslKey}>
            <Chip
              variant="outlined"
              onClick={async () => {
                let file = await selectOneFile();
                if (file) {
                  setSslKey(file);
                }
              }}
              label={
                'Key File(optional):' + (sslKey ? extractBaseName(sslKey) : '')
              }
            ></Chip>
          </Tooltip>
        </Stack>
      )}
    </Stack>
  );
}

function Database(props: {
  database;
  setDatabase;
  candidates: string[];
  onBlur?;
}) {
  let { database, setDatabase, candidates, onBlur } = props;
  const label = 'database';
  return (
    <Stack direction="row">
      {candidates.length > 0 && (
        <FormControl style={{ flex: 1 }}>
          <InputLabel> candidates </InputLabel>
          <Select
            value={database}
            label="candidates"
            onChange={(e) => setDatabase(e.target.value)}
          >
            {candidates.map((e) => (
              <MenuItem value={e} key={e}>
                {e}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      )}
      <TextField
        label={label}
        style={{ flex: 1 }}
        value={database}
        onBlur={onBlur}
        onChange={(e) => setDatabase(e.target.value)}
        slotProps={{
          input: {
            endAdornment: database && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setDatabase('');
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
    </Stack>
  );
}

function Schema(props: { schema; setSchema; candidates: string[] }) {
  let { schema, setSchema, candidates } = props;

  return (
    <Stack direction="row">
      {candidates.length > 0 && (
        <FormControl style={{ flex: 1 }}>
          <InputLabel>candidates</InputLabel>
          <Select
            value={schema}
            label="databs"
            onChange={(e) => setSchema(e.target.value)}
          >
            {candidates.map((e) => (
              <MenuItem key={e} value={e}>
                {e}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      )}
      <TextField
        style={{ width: '16rem', flex: 1 }}
        label="schema"
        value={schema}
        onChange={(e) => setSchema(e.target.value)}
        slotProps={{
          input: {
            endAdornment: schema && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setSchema('');
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
    </Stack>
  );
}

function AddConnection(props: { handleClose: () => void; config?: object }) {
  let { handleClose, config } = props;
  const isCopy = !!config;
  if (!config) {
    config = {};
  }
  const getConfig = (key: string, defaultValue: any = '') => {
    let v = config[key];
    if (!(key in config)) {
      v = defaultValue;
    }
    return v;
  };
  const [connectionType, setConnectionType] = useState(
    getConfig('conn_type', 'mysql')
  );
  const [connectionName, setConnectionName] = useState(getConfig('name'));
  const connectionNameDom = useRef(null);
  const [savePassword, setSavePassword] = useState(
    getConfig('save_password', true)
  );
  const [host, setHost] = useState(getConfig('host'));
  const [port, setPort] = useState('' + getConfig('port', 0));
  const [user, setUser] = useState(getConfig('username'));
  const [password, setPassword] = useState(getConfig('password', ''));
  const [database, setDatabase] = useState(getConfig('db'));
  const [readOnly, setReadOnly] = useState(getConfig('read_only', false));

  const [testLoading, setTestLoading] = useState(false);
  const [triggerReLoad, setTriggerReload] = useState<boolean | null>(null);
  const [path, setPath] = useState(getConfig('path'));
  const { showNotification } = useContext(NotificationContext);
  const [testStatus, setTestStatus] = useState<string | true | null>(null);
  const [trustCert, setTrustCert] = useState(getConfig('trust_cert', false));
  const [databaseCandidates, setDatabaseCandidates] = useState([]);
  const [schema, setSchema] = useState(getConfig('schema'));
  const [https, setHttps] = useState(getConfig('https', false));
  const [sslCa, setSslCa] = useState(getConfig('ssl_ca'));
  const [sslCert, setSslCert] = useState(getConfig('ssl_cert'));
  const [sslKey, setSslKey] = useState(getConfig('ssl_key'));
  const [sslEnable, setSslEnable] = useState(false);
  const toggleTriggerReload = () => {
    setTriggerReload((e) => !e);
  };
  const [connTimeout, setConnTimeout] = useState<string>(
    getConfig('conn_timeout')
  );

  const [libSqlModel, setLibSqlModal] = useState<'remote' | 'local'>(
    //@ts-ignore
    getConfig('source', 'local') // backend has name this to source
  );
  const [url, setUrl] = useState(getConfig('url'));
  const [replica, setReplica] = useState(getConfig('replica', false));
  const [authToken, setAuthToken] = useState(getConfig('auth_token'));
  const [schemaCandidates, setSchemesCandidates] = useState([]);

  const makeConfig = (): types.DBConnectionConfig | null => {
    const portInt = parseInt(port);
    const user_null = user ? user : null;
    const pass_null = password ? password : null;
    const db_null = database ? database : null;
    const schema_null = schema ? schema : null;
    const connTimeout_null = connTimeout ? parseInt(connTimeout) : null;
    switch (connectionType) {
      case DataSourceKind.FireBird:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          db: database,
          username: user,
          password: password,
          read_only: readOnly,
          name: connectionName,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.LibSql:
        return {
          conn_type: connectionType,
          replica: replica,
          source: libSqlModel,
          path,
          url: url,
          auth_token: authToken,
          read_only: readOnly,
          name: connectionName,
          conn_timeout: connTimeout_null,
        };

      case DataSourceKind.ClickHouse:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          https: https,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Mysql:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          save_password: savePassword,
          ssl_enable: sslEnable,
          ssl_ca: sslCa,
          ssl_cert: sslCert,
          ssl_key: sslKey,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Cassandra:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user_null,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.MariaDB:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.TiDB:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Postgres:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          schema: schema_null,
          save_password: savePassword,
          ssl_enable: sslEnable,
          ssl_ca: sslCa,
          ssl_cert: sslCert,
          ssl_key: sslKey,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Cockroach:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          schema: schema_null,
          save_password: savePassword,
          ssl_enable: sslEnable,
          ssl_ca: sslCa,
          ssl_cert: sslCert,
          ssl_key: sslKey,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Sqlite:
        if (!path) {
          showNotification('error', 'path is required');
          return null;
        }
        return {
          conn_type: connectionType,
          path,
          read_only: readOnly,
          name: connectionName,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.DuckDB:
        if (!path) {
          showNotification('error', 'path is required');
          return null;
        }
        return {
          conn_type: connectionType,
          path,
          db: db_null,
          schema: schema_null,
          read_only: readOnly,
          name: connectionName,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Redis:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Mongo:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.MsSql:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          conn_type: connectionType,
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user,
          password: password,
          trust_cert: trustCert,
          schema: null,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
      case DataSourceKind.Oracle:
        if (!host) {
          showNotification('error', 'host is required');
          return null;
        }
        return {
          name: connectionName,
          conn_type: connectionType,
          host: host,
          port: portInt,
          username: user,
          password: password,
          db: database,
          read_only: readOnly,
          schema: schema ? schema : null,
          save_password: savePassword,
          conn_timeout: connTimeout_null,
        };
    }
  };
  const handleTestConnection = async () => {
    const config = makeConfig();
    if (config) {
      console.log('test config:', config);
      setTestLoading(true);
      invoke('test_connection', { config })
        .then(() => {
          setTestStatus(true);
        })
        .catch((e) => {
          setTestStatus(errToString(e));
          showNotification('error', 'test connection failed', e);
        })
        .finally(() => {
          setTestLoading(false);
          setTimeout(() => {
            setTestStatus(null);
          }, msgTimeout);
        });
    }
  };

  useEffect(() => {
    if (isCopy) {
      return;
    }
    switch (connectionType) {
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
        setPort('3306');
        break;
      case DataSourceKind.TiDB:
        setPort('4000');
        break;
      case DataSourceKind.Sqlite:
        break;
      case DataSourceKind.Postgres:
        setPort('5432');
        break;
      case DataSourceKind.MsSql:
        setPort('1433');
        break;
      case DataSourceKind.Redis:
        setPort('6379');
        break;
      case DataSourceKind.Mongo:
        setPort('27017');
        break;
      case DataSourceKind.Oracle:
        setPort('1521');
        break;
      case DataSourceKind.Cassandra:
        setPort('9042');
        break;
      case DataSourceKind.ClickHouse:
        setPort('8123');
        break;
      case DataSourceKind.FireBird:
        setPort('3050');
        break;
      case DataSourceKind.Cockroach:
        setPort('26257');
        break;
    }
  }, [connectionType]);
  const handleSaveConnection = async () => {
    if (!connectionName) {
      showNotification('error', 'Connection Name is empty.');
      connectionNameDom.current.focus();
      return;
    }

    const config = makeConfig();
    if (!config) {
      return;
    }

    let connections = await appStore.get<types.DBConnectionConfig[]>(
      AppStoreKey.ConnectionConfig
    );
    if (!connections) {
      connections = [];
    }
    if (!validConnectionName(connectionName)) {
      showNotification('error', 'not valid connection name');
      connectionNameDom.current.focus();
      return;
    }
    if (connections.find((e) => e.name == connectionName)) {
      showNotification('error', 'name already been taken');
      connectionNameDom.current.focus();
      return;
    }
    if (password && !savePassword) {
      try {
        await invoke('save_entered_password', {
          name: connectionName,
          pass: password,
          conn: config,
        });
      } catch (e) {
        showNotification('error', 'save entered password failed', e);
        return;
      }
    }
    if (password && savePassword) {
      try {
        await invoke('save_password', {
          name: connectionName,
          pass: password,
        });
      } catch (e) {
        showNotification('error', 'save password failed', e);
        return;
      }
    }
    if (password) {
      config['password'] = '';
    }

    connections = [config, ...connections];
    appStore
      .set(AppStoreKey.ConnectionConfig, connections)
      .then((_) => {
        showNotification('success', 'Add Connection OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Add Connection failure', e);
      });
  };

  useEffect(() => {
    // first time we make sure it is a null value not a boolean
    if (triggerReLoad === null) {
      return;
    }
    let config = makeConfig();
    if (connectionType === DataSourceKind.Oracle) {
      // TODO oracle have bug right now
      return;
    }
    if (config) {
      invoke<string[]>('config_list_database_or_schema', { conn: config })
        .then((e) => {
          setDatabaseCandidates(e);
        })
        .catch((e) => {
          console.log('list database failed,', e);
          setDatabaseCandidates([]);
        });
    }
  }, [triggerReLoad]);

  useEffect(() => {
    if (
      database &&
      (connectionType === DataSourceKind.Postgres ||
        connectionType === DataSourceKind.Oracle ||
        connectionType === DataSourceKind.Cockroach)
    ) {
      let config = makeConfig();
      if (config) {
        invoke<string[]>('config_list_database_or_schema', { conn: config })
          .then((e) => {
            setSchemesCandidates(e);
          })
          .catch((e) => {
            console.log('list schemas failed,', e);
            setSchemesCandidates([]);
          });
      }
    }
  }, [database]);

  return (
    <Stack spacing={1}>
      <FormControl fullWidth>
        <InputLabel>Connection Type</InputLabel>
        <Select
          value={connectionType}
          label="Connection Type"
          onChange={(e) => setConnectionType(e.target.value as DataSourceKind)}
        >
          <MenuItem value={DataSourceKind.Mysql}>MySQL</MenuItem>
          <MenuItem value={DataSourceKind.MariaDB}>MariaDB</MenuItem>
          <MenuItem value={DataSourceKind.TiDB}>TIDB</MenuItem>
          <MenuItem value={DataSourceKind.Postgres}>PostgreSQL</MenuItem>
          <MenuItem value={DataSourceKind.Sqlite}>SQLite</MenuItem>
          <MenuItem value={DataSourceKind.Redis}>Redis</MenuItem>
          <MenuItem value={DataSourceKind.Mongo}>Mongo</MenuItem>
          <MenuItem value={DataSourceKind.Oracle}>Oracle</MenuItem>
          <MenuItem value={DataSourceKind.MsSql}>
            Sql Server(microsoft)
          </MenuItem>
          <MenuItem value={DataSourceKind.Cassandra}>Cassandra</MenuItem>
          <MenuItem value={DataSourceKind.ClickHouse}>Clickhouse</MenuItem>
          <MenuItem value={DataSourceKind.LibSql}> LibSql </MenuItem>
          <MenuItem value={DataSourceKind.FireBird}> FireBird </MenuItem>
          {/* <MenuItem value={DataSourceKind.DuckDB}> Duckdb </MenuItem> */}
          <MenuItem value={DataSourceKind.Cockroach}> Cockroach </MenuItem>
        </Select>
      </FormControl>

      {(connectionType === DataSourceKind.Mysql ||
        connectionType === DataSourceKind.MariaDB ||
        connectionType === DataSourceKind.TiDB) && (
        <>
          <HostAndPort
            host={host}
            port={port}
            onBlur={toggleTriggerReload}
            setHost={setHost}
            setPort={setPort}
          ></HostAndPort>
          <UserAndPassword
            user={user}
            onBlur={toggleTriggerReload}
            password={password}
            setUser={setUser}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            database={database}
            candidates={databaseCandidates}
            setDatabase={setDatabase}
          ></Database>
          <EnableSSlCert
            enableSsl={sslEnable}
            setEnableSsl={setSslEnable}
            sslCa={sslCa}
            setSslCa={setSslCa}
            sslCert={sslCert}
            setSslCert={setSslCert}
            sslKey={sslKey}
            setSslKey={setSslKey}
          ></EnableSSlCert>
        </>
      )}

      {connectionType === DataSourceKind.LibSql && (
        <LibSql
          url={url}
          setUrl={setUrl}
          path={path}
          setPath={setPath}
          replica={replica}
          setReplica={setReplica}
          authToken={authToken}
          setAuthToken={setAuthToken}
          setLibSqlModal={setLibSqlModal}
          libSqlModel={libSqlModel}
        ></LibSql>
      )}

      {connectionType === DataSourceKind.FireBird && (
        <>
          <HostAndPort
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
            onBlur={toggleTriggerReload}
          ></HostAndPort>
          <UserAndPassword
            user={user}
            password={password}
            setUser={setUser}
            onBlur={toggleTriggerReload}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={[]}
            database={database}
            setDatabase={setDatabase}
            onBlur={toggleTriggerReload}
          ></Database>
        </>
      )}

      {(connectionType === DataSourceKind.Postgres ||
        connectionType === DataSourceKind.Cockroach) && (
        <>
          <HostAndPort
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
            onBlur={toggleTriggerReload}
          ></HostAndPort>
          <UserAndPassword
            user={user}
            password={password}
            setUser={setUser}
            onBlur={toggleTriggerReload}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
            onBlur={toggleTriggerReload}
          ></Database>
          {database && (
            <Schema
              schema={schema}
              setSchema={setSchema}
              candidates={schemaCandidates}
            ></Schema>
          )}
          <EnableSSlCert
            enableSsl={sslEnable}
            setEnableSsl={setSslEnable}
            sslCa={sslCa}
            setSslCa={setSslCa}
            sslCert={sslCert}
            setSslCert={setSslCert}
            sslKey={sslKey}
            setSslKey={setSslKey}
          ></EnableSSlCert>
        </>
      )}
      {connectionType === DataSourceKind.ClickHouse && (
        <>
          <HostAndPort
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
            onBlur={toggleTriggerReload}
          ></HostAndPort>
          <UserAndPassword
            user={user}
            password={password}
            setUser={setUser}
            onBlur={toggleTriggerReload}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
            onBlur={toggleTriggerReload}
          ></Database>
          <Https https={https} setHttps={setHttps}></Https>
        </>
      )}

      {connectionType === DataSourceKind.Cassandra && (
        <>
          <HostAndPort
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
            onBlur={toggleTriggerReload}
          ></HostAndPort>
          <UserAndPassword
            user={user}
            password={password}
            setUser={setUser}
            onBlur={toggleTriggerReload}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
            onBlur={toggleTriggerReload}
          ></Database>
        </>
      )}
      {connectionType === DataSourceKind.Redis && (
        <>
          <HostAndPort
            host={host}
            port={port}
            onBlur={toggleTriggerReload}
            setHost={setHost}
            setPort={setPort}
          ></HostAndPort>
          <UserAndPassword
            onBlur={toggleTriggerReload}
            user={user}
            password={password}
            setUser={setUser}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}
      {connectionType === DataSourceKind.Mongo && (
        <>
          <HostAndPort
            onBlur={toggleTriggerReload}
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
          ></HostAndPort>
          <UserAndPassword
            onBlur={toggleTriggerReload}
            user={user}
            password={password}
            setUser={setUser}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}
      {connectionType === DataSourceKind.MsSql && (
        <>
          <HostAndPort
            onBlur={toggleTriggerReload}
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
          ></HostAndPort>
          <UserAndPassword
            onBlur={toggleTriggerReload}
            user={user}
            password={password}
            setUser={setUser}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            database={database}
            setDatabase={setDatabase}
            candidates={databaseCandidates}
          ></Database>
          <TrustCert
            trustCert={trustCert}
            setTrustCert={setTrustCert}
          ></TrustCert>
        </>
      )}

      {connectionType === DataSourceKind.Oracle && (
        <>
          <Typography
            color="info"
            style={{ cursor: 'pointer' }}
            onClick={() => {
              openUrl(
                'https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html'
              );
            }}
          >
            oracle need extra driver
          </Typography>
          <HostAndPort
            onBlur={toggleTriggerReload}
            host={host}
            port={port}
            setHost={setHost}
            setPort={setPort}
          ></HostAndPort>
          <UserAndPassword
            onBlur={toggleTriggerReload}
            user={user}
            password={password}
            setUser={setUser}
            setPassword={setPassword}
            savePassword={savePassword}
            setSavePassword={setSavePassword}
          ></UserAndPassword>
          <Database
            database={database}
            setDatabase={setDatabase}
            candidates={databaseCandidates}
          ></Database>
          {database && (
            <Schema
              schema={schema}
              setSchema={setSchema}
              candidates={schemaCandidates}
            ></Schema>
          )}
        </>
      )}
      {connectionType === DataSourceKind.Sqlite && (
        <SelectPath
          path={path}
          setPath={setPath}
          required
          label="sqlite database file"
        ></SelectPath>
      )}
      {connectionType === DataSourceKind.DuckDB && (
        <>
          <SelectPath
            path={path}
            setPath={setPath}
            label="duckdb database file"
            required
          ></SelectPath>
          <Database
            database={database}
            candidates={databaseCandidates}
            onBlur={toggleTriggerReload}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}

      <Stack direction="row" style={{ alignItems: 'flex-start' }} spacing={2}>
        <TextField
          label="Connection Name"
          value={connectionName}
          inputRef={connectionNameDom}
          error={connectionName ? !validConnectionName(connectionName) : false}
          required
          onChange={(e) => setConnectionName(e.target.value)}
          slotProps={{
            input: {
              endAdornment: connectionName && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnectionName('');
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          style={{ flex: 1 }}
        />

        <TextField
          label="connection timeout"
          value={connTimeout}
          style={{ flex: 1 }}
          onChange={(e) => setConnTimeout(e.target.value)}
          slotProps={{
            input: {
              endAdornment: connTimeout && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnTimeout('');
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
          error={connTimeout ? isNaN(parseInt(connTimeout)) : false}
          helperText="milliseconds to wait for connect"
        />
        <FormControlLabel
          style={{ width: '180px' }}
          control={
            <Switch
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
      </Stack>
      <Divider />
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          color="primary"
          onClick={() => {
            handleSaveConnection();
          }}
        >
          save
        </Button>
        <Button
          variant="outlined"
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
          loading={testLoading}
        >
          test
        </Button>
        {testStatus === true && (
          <Typography color="success">Test OK</Typography>
        )}
        {typeof testStatus == 'string' && (
          <Typography color="error"> {testStatus} </Typography>
        )}
      </Stack>
    </Stack>
  );
}

function LibSql(props: {
  replica: boolean;
  setReplica;
  path: string;
  setPath;
  url: string;
  setUrl;
  authToken: string;
  setAuthToken;
  libSqlModel;
  setLibSqlModal;
}) {
  const {
    replica,
    setReplica,
    path,
    setPath,
    url,
    setUrl,
    authToken,
    setAuthToken,
    libSqlModel,
    setLibSqlModal,
  } = props;
  let models = ['remote', 'local'];
  return (
    <>
      <RadioGroup
        row
        onChange={(e) => {
          setLibSqlModal(e.target.value);
        }}
        value={libSqlModel}
      >
        {models.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>

      {libSqlModel === 'local' && (
        <SelectPath
          path={path}
          setPath={setPath}
          required
          label="libsql database file"
        ></SelectPath>
      )}
      {libSqlModel === 'remote' && (
        <>
          <Stack direction="row" spacing={1}>
            <TextField
              value={url}
              label="url"
              onChange={(e) => {
                setUrl(e.target.value);
              }}
              fullWidth
              slotProps={{
                input: {
                  endAdornment: url && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setUrl('');
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
            <TextField
              fullWidth
              value={authToken}
              label="auth token "
              onChange={(e) => {
                setAuthToken(e.target.value);
              }}
              slotProps={{
                input: {
                  endAdornment: authToken && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setAuthToken('');
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
          </Stack>
          <Stack direction="row" style={{ alignItems: 'center' }}>
            <Checkbox
              value={replica}
              onChange={(e) => {
                setReplica(e.target.checked);
              }}
            ></Checkbox>
            replica
            {replica && (
              <SelectPath
                label="replica"
                path={path}
                setPath={setPath}
                required={false}
              ></SelectPath>
            )}
          </Stack>
        </>
      )}
    </>
  );
}

// TODO we can support more
function validConnectionName(str: string) {
  const regex = /^[a-zA-Z0-9_$-]+$/;
  return regex.test(str);
}

export { AddConnection };
