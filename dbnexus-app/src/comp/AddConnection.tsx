import { useContext, useEffect, useState } from 'react';

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
  CircularProgress,
  Alert,
  Divider,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { types } from './ApiStruct';
import { open } from '@tauri-apps/plugin-dialog';

import { invoke } from '@tauri-apps/api/core';
import { appStore, AppStoreKey, msgTimeout, progressBarSize } from './Common';

import AttachFileIcon from '@mui/icons-material/AttachFile';
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
        margin="normal"
        fullWidth
        label="Host"
        value={host}
        onBlur={onBlur}
        sx={{ mr: 1 }}
        onChange={(e) => setHost(e.target.value)}
      />
      <TextField
        size="small"
        margin="normal"
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

function Sqlite(props: { path; setPath }) {
  let { path, setPath } = props;
  return (
    <Stack direction="row" sx={{ alignItems: 'flex-end' }}>
      <Button
        onClick={() => {
          selectOneFile().then((e) => {
            setPath(e);
          });
        }}
      >
        <AttachFileIcon></AttachFileIcon>
      </Button>
      <Typography>path:{path}</Typography>
    </Stack>
  );
}

function UserAndPassword(props: {
  user;
  password;
  setUser;
  setPassword;
  onBlur;
}) {
  let { user, password, setUser, setPassword, onBlur } = props;

  return (
    <Stack direction="row">
      <TextField
        size="small"
        margin="normal"
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onBlur={onBlur}
        onChange={(e) => setUser(e.target.value)}
      />
      <TextField
        size="small"
        margin="normal"
        fullWidth
        label="Password"
        onBlur={onBlur}
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
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

function Database(props: {
  database;
  setDatabase;
  candidates: string[];
  onBlur?;
}) {
  let { database, setDatabase, candidates, onBlur } = props;

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
          size="small"
          margin="normal"
          label="database"
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
          margin="normal"
          label="schema"
          value={schema}
          onChange={(e) => setSchema(e.target.value)}
        />
      </Stack>
    </Stack>
  );
}

export default function AddConfig(props: { handleClose: () => void }) {
  let { handleClose } = props;

  const [connectionType, setConnectionType] = useState(DataSourceKind.Mysql);
  const [connectionName, setConnectionName] = useState('');
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('3306');
  const [user, setUser] = useState('');
  const [password, setPassword] = useState('');
  const [database, setDatabase] = useState('');
  const [readOnly, setReadOnly] = useState(false);
  //@ts-ignore
  const [useSSL, setUseSSL] = useState(false);
  const [testLoading, setTestLoading] = useState(false);
  const [triggerReLoad, setTriggerReload] = useState(false);
  const [path, setPath] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [testOk, setTestOk] = useState(false);
  const [trustCert, setTrustCert] = useState(false);
  const [databaseCandidates, setDatabaseCandidats] = useState([]);
  const [schema, setSchema] = useState('');
  const toggleTriggerReload = () => {
    setTriggerReload((e) => !e);
  };
  const [schemaCandidates, setSchemesCandidats] = useState([]);

  const makeConfig = (): types.DBConnectionConfig | null => {
    let portInt = parseInt(port);
    if (isNaN(portInt)) {
      showNotification('error', 'port is not a number');
      return null;
    }
    let user_null = user ? user : null;
    let pass_null = password ? password : null;
    let db_null = database ? database : null;
    let schema_null = schema ? schema : null;
    switch (connectionType) {
      case DataSourceKind.Mysql:
        return {
          conn_type: 'mysql',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.MariaDB:
        return {
          conn_type: 'mariadb',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.TiDB:
        return {
          conn_type: 'tidb',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.Postgres:
        return {
          conn_type: 'postgres',
          host: host,
          port: portInt,
          username: user,
          password: pass_null,
          db: db_null,
          read_only: readOnly,
          name: connectionName,
          schema: schema_null,
        };

      case DataSourceKind.Sqlite:
        return {
          conn_type: 'sqlite',
          path: path,
          read_only: readOnly,
          name: connectionName,
        };
      case DataSourceKind.Redis:
        return {
          conn_type: 'redis',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
        };
      case DataSourceKind.Mongo:
        return {
          conn_type: 'mongo',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user_null,
          password: pass_null,
          connect_timeout: null,
        };
      case DataSourceKind.MsSql:
        return {
          conn_type: 'sqlserver',
          read_only: readOnly,
          name: connectionName,
          host: host,
          port: portInt,
          db: db_null,
          username: user,
          password: password,
          trust_cert: trustCert,
          schema: null,
        };
      case DataSourceKind.Oracle:
        return {
          name: connectionName,
          conn_type: 'oracle',
          host: host,
          port: portInt,
          username: user,
          password: password,
          db: database,
          read_only: readOnly,
          schema: schema ? schema : null,
          call_timeout: null,
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
    }
    setUser('');
    setPassword('');
    setDatabase('');
    setPassword('');
    setDatabaseCandidats([]);
    setSchemesCandidats([]);
    setSchema('');
  }, [connectionType]);
  const handleSaveConnection = async () => {
    if (!connectionName) {
      showNotification('error', 'Connection Name is empty.');
      return;
    }
    const config = makeConfig();
    if (!config) {
      return;
    }
    if (password) {
      try {
        await invoke('save_password', { name: connectionName, pass: password });
      } catch (e) {
        showNotification('error', 'save password failed', e);
        return;
      }
      //@ts-ignore
      config.password = '';
    }

    let old = await (await appStore()).get(AppStoreKey.ConnectionConfig);
    if (!old) {
      old = [];
    }
    //@ts-ignore
    old = [config, ...old];
    (await appStore())
      .set(AppStoreKey.ConnectionConfig, old)
      .then((_) => {
        showNotification('success', 'Add Connection OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Add Connection failure', e);
      });
  };

  useEffect(() => {
    switch (connectionType) {
      case DataSourceKind.MsSql:
      case DataSourceKind.Postgres:
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.Redis:
      case DataSourceKind.Mongo:
        let config = makeConfig();
        if (config) {
          invoke<string[]>('config_list_database_or_schema', { conn: config })
            .then((e) => {
              setDatabaseCandidats(e);
            })
            .catch((e) => {
              console.log('list database failed,', e);
              setDatabaseCandidats([]);
            });
        }
    }
  }, [triggerReLoad]);

  useEffect(() => {
    if (database && connectionType == DataSourceKind.Postgres) {
      let config = makeConfig();
      if (config) {
        invoke<string[]>('config_list_database_or_schema', { conn: config })
          .then((e) => {
            setSchemesCandidats(e);
          })
          .catch((e) => {
            console.log('list database failed,', e);
            setSchemesCandidats([]);
          });
      }
    }
  }, [database]);

  return (
    <Stack>
      <Typography variant="h6" gutterBottom>
        New Connection
      </Typography>
      <FormControl fullWidth margin="normal">
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
            Sql server(microsoft)
          </MenuItem>
        </Select>
      </FormControl>

      {(connectionType == DataSourceKind.Mysql ||
        connectionType == DataSourceKind.MariaDB ||
        connectionType == DataSourceKind.TiDB) && (
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
          ></UserAndPassword>
          <Database
            database={database}
            candidates={databaseCandidates}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}
      {connectionType == DataSourceKind.Postgres && (
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
        </>
      )}
      {connectionType == DataSourceKind.Redis && (
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
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}
      {connectionType == DataSourceKind.Mongo && (
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
          ></UserAndPassword>
          <Database
            candidates={databaseCandidates}
            database={database}
            setDatabase={setDatabase}
          ></Database>
        </>
      )}
      {connectionType == DataSourceKind.MsSql && (
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

      {connectionType == DataSourceKind.Oracle && (
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
      {connectionType == DataSourceKind.Sqlite && (
        <Sqlite path={path} setPath={setPath}></Sqlite>
      )}
      <Stack direction="row">
        <Button
          variant="outlined"
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
        >
          Test
        </Button>
        {testOk && <Alert severity="success"> Test OK </Alert>}
        {testLoading && <CircularProgress size={progressBarSize} />}
      </Stack>
      <Stack direction="row" alignItems="flex-start" spacing={2}>
        <FormControlLabel
          control={
            <Switch
              checked={readOnly}
              onChange={(e) => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
        <TextField
          size="small"
          margin="normal"
          label="Connection Name"
          value={connectionName}
          fullWidth
          error={!connectionName}
          required
          onChange={(e) => setConnectionName(e.target.value)}
          helperText="Enter a name for this connection"
        />
      </Stack>
      <Divider></Divider>
      <Stack direction="row" spacing={1} alignContent="flex-start">
        <Button
          variant="contained"
          color="primary"
          onClick={() => {
            handleSaveConnection();
          }}
        >
          Save
        </Button>
      </Stack>
    </Stack>
  );
}
