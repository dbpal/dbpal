import { useContext, useEffect, useRef, useState } from 'react';

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
  Alert,
  Divider,
  RadioGroup,
  Radio,
  Checkbox,
  Chip,
  Tooltip,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { types } from './ApiStruct';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { appStore, AppStoreKey, extractBaseName, msgTimeout } from './Common';
import { DataSourceKind } from './helper';

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
        size="small"
        fullWidth
        label="Host"
        value={host}
        onBlur={onBlur}
        sx={{ mr: 1 }}
        onChange={(e) => setHost(e.target.value)}
      />
      <TextField
        size="small"
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

function SelectPath(props: { path; setPath }) {
  let { path, setPath } = props;
  return (
    <Stack direction="row" sx={{ alignItems: 'center' }}>
      <Chip
        label={`path:` + path}
        onClick={() => {
          selectOneFile().then((e) => {
            setPath(e);
          });
        }}
        color="primary"
      ></Chip>
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

  return (
    <Stack direction="row">
      <TextField
        size="small"
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onBlur={onBlur}
        onChange={(e) => setUser(e.target.value)}
      />
      <TextField
        size="small"
        fullWidth
        label="Password"
        onBlur={onBlur}
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />

      <div style={{ width: '30rem' }}>
        <Checkbox
          checked={savePassword}
          onChange={(e) => {
            setSavePassword(e.target.checked);
          }}
          name="save password"
        />
        save password
      </div>
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
    <FormControlLabel
      control={
        <Switch
          checked={trustCert}
          onChange={(e) => setTrustCert(e.target.checked)}
        />
      }
      label="Trust Cert"
    />
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
    <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
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
        <>
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
              label={'CA Cert:' + (sslCa ? extractBaseName(sslCa) : '')}
            ></Chip>
          </Tooltip>
          <Tooltip title={sslCert}>
            <Chip
              onClick={async () => {
                let file = await selectOneFile();
                if (file) {
                  setSslCert(file);
                }
              }}
              label={'Certificate:' + (sslCert ? extractBaseName(sslCert) : '')}
            ></Chip>
          </Tooltip>

          <Tooltip title={sslKey}>
            <Chip
              onClick={async () => {
                let file = await selectOneFile();
                if (file) {
                  setSslKey(file);
                }
              }}
              label={'Key File:' + (sslKey ? extractBaseName(sslKey) : '')}
            ></Chip>
          </Tooltip>
        </>
      )}
    </Stack>
  );
}

function Database(props: {
  database;
  setDatabase;
  candidates: string[];
  onBlur?;
  lable?: string;
}) {
  let { database, setDatabase, candidates, onBlur, lable } = props;

  if (!lable) {
    lable = 'database';
  }
  return (
    <Stack>
      <Stack direction="row" style={{ flexWrap: 'wrap' }}>
        {candidates.map((e) => (
          <Button
            key={e}
            onClick={() => {
              setDatabase(e);
            }}
          >
            {e}
          </Button>
        ))}
      </Stack>

      <Stack direction="row">
        <TextField
          fullWidth
          size="small"
          label={lable}
          value={database}
          onBlur={onBlur}
          onChange={(e) => setDatabase(e.target.value)}
        />
      </Stack>
    </Stack>
  );
}

function Schema(props: { schema; setSchema; candidates: string[] }) {
  let { schema, setSchema, candidates } = props;

  return (
    <Stack>
      <Stack direction="row" style={{ flexWrap: 'wrap' }}>
        {candidates.map((e) => (
          <Button
            key={e}
            onClick={() => {
              setSchema(e);
            }}
          >
            {e}
          </Button>
        ))}
      </Stack>

      <Stack direction="row">
        <TextField
          size="small"
          label="schema"
          value={schema}
          onChange={(e) => setSchema(e.target.value)}
        />
      </Stack>
    </Stack>
  );
}

function AddConnection(props: { handleClose: () => void; config?: object }) {
  let { handleClose, config } = props;
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
  const [password, setPassword] = useState('');
  const [database, setDatabase] = useState(getConfig('db'));
  const [readOnly, setReadOnly] = useState(getConfig('read_only', false));

  const [testLoading, setTestLoading] = useState(false);
  const [triggerReLoad, setTriggerReload] = useState(false);
  const [path, setPath] = useState(getConfig('path'));
  const { showNotification } = useContext(NotificationContext);
  const [testOk, setTestOk] = useState(false);
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
  const [libSqlModel, setlibSqlModal] = useState<'remote' | 'local'>(
    //@ts-ignore
    getConfig('source') // backend has name this to source
  );
  const [url, setUrl] = useState(getConfig('url'));
  const [replica, setReplica] = useState(getConfig('replica', false));
  const [authToken, setAuthToken] = useState(getConfig('auth_token'));
  const [schemaCandidates, setSchemesCandidats] = useState([]);
  const makeConfig = (): types.DBConnectionConfig | null => {
    let portInt = parseInt(port);
    let user_null = user ? user : null;
    let pass_null = password ? password : null;
    let db_null = database ? database : null;
    let schema_null = schema ? schema : null;
    switch (connectionType) {
      case DataSourceKind.FireBird:
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
        };
      case DataSourceKind.LibSql:
        return {
          conn_type: connectionType,
          replica: replica,
          source: libSqlModel,
          path: path,
          url: url,
          auth_token: authToken,
          read_only: readOnly,
          name: connectionName,
        };

      case DataSourceKind.ClickHouse:
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
        };
      case DataSourceKind.Mysql:
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
        };
      case DataSourceKind.Cassandra:
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
        };
      case DataSourceKind.MariaDB:
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
        };
      case DataSourceKind.TiDB:
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
        };
      case DataSourceKind.Postgres:
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
        };
      case DataSourceKind.Cockroach:
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
        };
      case DataSourceKind.Sqlite:
        return {
          conn_type: connectionType,
          path: path,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.DuckDB:
        return {
          conn_type: connectionType,
          path: path,
          db: db_null,
          schema: schema_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.Redis:
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
        };
      case DataSourceKind.Mongo:
        return {
          conn_type: connectionType,
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
          call_timeout: null,
          save_password: savePassword,
        };
      case DataSourceKind.MsSql:
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
        };
      case DataSourceKind.Oracle:
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
          call_timeout: null,
          save_password: savePassword,
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
          setTestLoading(false);
          setTestOk(true);
          setTimeout(() => {
            setTestOk(false);
          }, msgTimeout);
        })
        .catch((e) => {
          setTestLoading(false);
          showNotification('error', 'test connection failed', e);
        });
    }
  };

  useEffect(() => {
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

    let connections = await (
      await appStore()
    ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
    if (!connections) {
      connections = [];
    }
    if (connectionName.includes('/')) {
      showNotification('error', "connection name can't contains '/'");
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
        });
      } catch (e) {
        showNotification('error', 'save password failed', e);
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
    (await appStore())
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
    let config = makeConfig();
    if (connectionType === DataSourceKind.Oracle) {
      // TODO oracle have bug rigthnow
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
            setSchemesCandidats(e);
          })
          .catch((e) => {
            console.log('list schemas failed,', e);
            setSchemesCandidats([]);
          });
      }
    }
  }, [database]);

  return (
    <Stack spacing={1}>
      <Typography variant="h6">New Connection</Typography>
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
          <MenuItem value={DataSourceKind.Oracle}>Oracel</MenuItem>
          <MenuItem value={DataSourceKind.MsSql}>
            Sql Server(microsoft)
          </MenuItem>
          <MenuItem value={DataSourceKind.Cassandra}>Cassandra</MenuItem>
          <MenuItem value={DataSourceKind.ClickHouse}>Clickhouse</MenuItem>
          <MenuItem value={DataSourceKind.LibSql}> LibSql </MenuItem>
          <MenuItem value={DataSourceKind.FireBird}> FireBird </MenuItem>
          <MenuItem value={DataSourceKind.DuckDB}> Duckdb </MenuItem>
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
          setlibSqlModal={setlibSqlModal}
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

          {database && (
            <Schema
              schema={schema}
              setSchema={setSchema}
              candidates={schemaCandidates}
            ></Schema>
          )}
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
            lable="key space"
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
        <SelectPath path={path} setPath={setPath}></SelectPath>
      )}
      {connectionType === DataSourceKind.DuckDB && (
        <>
          <SelectPath path={path} setPath={setPath}></SelectPath>
          <Database
            database={database}
            candidates={databaseCandidates}
            onBlur={toggleTriggerReload}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}

      <Stack direction="row" style={{ alignItems: 'center' }} spacing={2}>
        <TextField
          size="small"
          label="Connection Name"
          value={connectionName}
          fullWidth
          inputRef={connectionNameDom}
          required
          onChange={(e) => setConnectionName(e.target.value)}
        />
        <FormControlLabel
          style={{ width: '180px' }}
          control={
            <Switch
              size="small"
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
      </Stack>
      <Divider></Divider>
      <Stack direction="row" spacing={1} alignContent="center">
        <Button
          variant="contained"
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
        {testOk && <Alert severity="success"> Test OK </Alert>}
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
  setlibSqlModal;
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
    setlibSqlModal,
  } = props;
  let models = ['remote', 'local'];
  return (
    <>
      <RadioGroup
        row
        onChange={(e) => {
          setlibSqlModal(e.target.value);
        }}
        value={libSqlModel}
      >
        {models.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>

      {libSqlModel === 'local' && (
        <SelectPath path={path} setPath={setPath}></SelectPath>
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
              size="small"
            ></TextField>
            <TextField
              fullWidth
              value={authToken}
              size="small"
              label="auth token "
              onChange={(e) => {
                setAuthToken(e.target.value);
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
            {replica && <SelectPath path={path} setPath={setPath}></SelectPath>}
          </Stack>
        </>
      )}
    </>
  );
}

export { AddConnection };
