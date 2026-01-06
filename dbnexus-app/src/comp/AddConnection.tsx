import {
  createContext,
  RefObject,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import ClearIcon from "@mui/icons-material/Clear";
import {
  TextField,
  Button,
  FormControlLabel,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Stack,
  Divider,
  Checkbox,
  Chip,
  Tooltip,
  InputAdornment,
  IconButton,
  Autocomplete,
} from "@mui/material";
import AttachFileIcon from "@mui/icons-material/AttachFile";
import VisibilityIcon from "@mui/icons-material/Visibility";
import { NotificationContext } from "./Notification";
import { types } from "./ApiStruct";
import { message, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import {
  ConnectionHandler,
  connectionChangeEvent,
  extractBaseName,
  PassWordManager,
} from "./Common";
import { DataSourceKind } from "./helper";
import { openUrl } from "@tauri-apps/plugin-opener";
import { platform } from "@tauri-apps/plugin-os";
import { StackRowCenter, UrlDocument } from "./CommonComp2";
import { emit } from "@tauri-apps/api/event";

function HostAndPort(props: { host; port; setHost; setPort }) {
  const { host, port, setHost, setPort } = props;
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="Host"
        value={host}
        sx={{ mr: 1 }}
        onChange={e => setHost(e.target.value)}
        slotProps={{
          input: {
            endAdornment: host && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setHost("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        required
      />
      <TextField
        fullWidth
        label="port"
        error={isNaN(parseInt(port))}
        value={port}
        onChange={e => setPort(e.target.value)}
      />
    </Stack>
  );
}

function SelectPath(props: { path; setPath; label: string; required: boolean }) {
  const { path, setPath, label, required } = props;
  return (
    <Stack direction="row" sx={{ alignItems: "center" }}>
      <TextField
        value={path}
        onChange={e => {
          setPath(e.target.value);
        }}
        style={{ flex: 1 }}
        label={label}
        required={required}
        slotProps={{
          input: {
            endAdornment: (
              <>
                {path && (
                  <InputAdornment position="end">
                    <IconButton
                      onClick={() => {
                        setPath("");
                      }}
                      edge="end"
                    >
                      <ClearIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                )}
                <IconButton
                  onClick={() => {
                    open({ multiple: false }).then(e => {
                      setPath(e);
                    });
                  }}
                  color="primary"
                >
                  <AttachFileIcon />
                </IconButton>
              </>
            ),
          },
        }}
      />
    </Stack>
  );
}

function UserAndPassword(props: {
  user: string;
  password: string;
  setUser;
  setPassword;
  savePassword;
  setSavePassword;
}) {
  const { user, password, setUser, setPassword, savePassword, setSavePassword } = props;
  const [passwordVisible, setPasswordVisible] = useState(false);
  return (
    <Stack direction="row">
      <TextField
        fullWidth
        label="User"
        value={user}
        sx={{ mr: 1 }}
        onChange={e => setUser(e.target.value)}
        slotProps={{
          input: {
            endAdornment: user && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setUser("");
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
        type={passwordVisible ? "text" : "password"}
        value={password}
        onChange={e => setPassword(e.target.value)}
        slotProps={{
          input: {
            endAdornment: !passwordVisible &&
              password != "" &&
              platform() != "windows" && ( // window 已经有了这个功能了
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
          width: "31rem",
          justifyContent: "center",
          alignItems: "center",
        }}
        direction="row"
      >
        <Checkbox
          checked={savePassword}
          onChange={e => {
            setSavePassword(e.target.checked);
          }}
          name="save password"
        />
        save password
      </Stack>
    </Stack>
  );
}

function Https(props: { https; setHttps }) {
  const { https, setHttps } = props;
  return (
    <FormControlLabel
      control={<Checkbox checked={https} onChange={e => setHttps(e.target.checked)} />}
      label="HTTPS"
    />
  );
}

function SqlServerCert(props: { data: SqlServerCertData; setData }) {
  const { data, setData } = props;
  return (
    <StackRowCenter>
      <FormControlLabel
        control={
          <Checkbox
            checked={data.trust_cert}
            onChange={e =>
              setData({
                ...data,
                trust_cert: e.target.checked,
              } as SqlServerCertData)
            }
          />
        }
        label="Trust Cert"
      />
      <Chip
        label="?"
        onClick={() => {
          openUrl(
            "https://docs.rs/tiberius/0.12.3/tiberius/struct.Config.html#method.trust_cert",
          );
        }}
      />
    </StackRowCenter>
  );
}

interface SqlServerCertData {
  trust_cert: boolean;
  cert_ca: string | null;
}

function SslFiles(props: { files: types.SSLFiles; setFiles; enabled: boolean }) {
  const { files, setFiles, enabled } = props;

  return (
    enabled && (
      <>
        <Stack direction="row" style={{ flexWrap: "wrap" }}>
          <Tooltip
            title={files.ssl_ca}
            onClick={async () => {
              const file = await open({ multiple: false });
              if (file) {
                setFiles({ ...files, ssl_ca: file });
              }
            }}
          >
            <Chip
              label={"CA Cert:" + (files.ssl_ca ? extractBaseName(files.ssl_ca) : "")}
              variant="outlined"
            />
          </Tooltip>
          <Tooltip title={files.ssl_cert}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await open({ multiple: false });
                if (file) {
                  setFiles({ ...files, ssl_cert: file });
                }
              }}
              label={
                "Certificate:" + (files.ssl_cert ? extractBaseName(files.ssl_cert) : "")
              }
            />
          </Tooltip>
          <Tooltip title={files.ssl_key}>
            <Chip
              variant="outlined"
              onClick={async () => {
                const file = await open({ multiple: false });
                if (file) {
                  setFiles({ ...files, ssl_key: file });
                }
              }}
              label={
                "Key File:" + (files.ssl_key ? extractBaseName(files.ssl_key) : "")
              }
            />
          </Tooltip>
        </Stack>
      </>
    )
  );
}

function PGSSL(props: { ssl: types.PgSsl; setSsl: any }) {
  const { ssl, setSsl } = props;
  const modes = [
    "disable",
    "allow",
    "prefer",
    "require",
    "verifyca",
    "verifyfull",
  ] as types.PgSslMode[];

  return (
    <Stack direction="row" style={{ alignItems: "center", flexWrap: "wrap" }}>
      <FormControl style={{ width: "12rem" }}>
        <InputLabel> ssl mode </InputLabel>
        <Select
          value={ssl.ssl_mode}
          label="ssl mode"
          onChange={e => setSsl({ ...ssl, ssl_mode: e.target.value } as types.PgSsl)}
        >
          {modes.map(e => (
            <MenuItem value={e} key={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
      <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/postgres/enum.PgSslMode.html" />

      <SslFiles enabled={ssl.ssl_mode != "disable"} files={ssl} setFiles={setSsl} />
    </Stack>
  );
}
function MysqlSSl(props: { ssl: types.MysqlSsl; setSsl: any }) {
  const { ssl, setSsl } = props;
  const modes = [
    "disabled",
    "preferred",
    "required",
    "verifyca",
    "verifyidentity",
  ] as types.MySqlSslMode[];
  return (
    <Stack direction="row" style={{ alignItems: "center", flexWrap: "wrap" }}>
      <FormControl style={{ width: "12rem" }}>
        <InputLabel> ssl mode </InputLabel>
        <Select
          value={ssl.ssl_mode}
          label="ssl mode"
          onChange={e => setSsl({ ...ssl, ssl_mode: e.target.value } as types.MysqlSsl)}
        >
          {modes.map(e => (
            <MenuItem value={e} key={e}>
              {e}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
      <UrlDocument url="https://docs.rs/sqlx/latest/sqlx/mysql/enum.MySqlSslMode.html" />

      <SslFiles enabled={ssl.ssl_mode != "disabled"} files={ssl} setFiles={setSsl} />
    </Stack>
  );
}

function DbSchemaCadidates(props: {
  name;
  setName;
  label: string;
  makeConfig: MakeConfig;
}) {
  const { name, setName, label, makeConfig } = props;
  const [candidates, setCadidates] = useState([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const handleReload = () => {
    const config = makeConfig();
    if (!config.name) {
      config.name = "test_conn";
    }
    setLoading(true);
    invoke<string[]>("try_list_database_schema_candidates", { conn: config })
      .then(e => {
        showNotification("success", "fetch candidates success");
        setCadidates(e);
      })
      .catch(e => {
        showNotification("warning", `list ${label} failed`, e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack direction="row">
      <Autocomplete
        options={candidates}
        style={{ flex: 1 }}
        value={name}
        renderInput={params => <TextField {...params} label={`${label} candidates`} />}
        freeSolo
        onChange={(_, name) => {
          setName(name);
        }}
        onInputChange={(_, value) => {
          setName(value);
        }}
      />

      <TextField
        label={label}
        style={{ flex: 1 }}
        value={name}
        onChange={e => setName(e.target.value)}
        slotProps={{
          input: {
            endAdornment: name && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setName("");
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
      <Button onClick={handleReload} loading={loading}>
        load candidates
      </Button>
    </Stack>
  );
}

type MakeConfig = () => types.DBConnectionConfig;

function Mysql(props: { config; kind: DataSourceKind }) {
  const { config, kind } = props;
  const { connectionName, connTimeOut, readOnly } = useContext(AddConnectionContext);
  let defaultPort = 3306;
  if (kind == DataSourceKind.TiDB) {
    defaultPort = 4000;
  }
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || defaultPort) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [ssl, setSsl] = useState<types.MysqlSsl>({
    ssl_mode: config?.ssl_mode || "preferred",
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [savePassword, setSavePassword] = useState(getSavePassword(config));

  function makeConfig(): types.DBConnectionConfig {
    const x: types.MysqlDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      db: database ? database : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      ...ssl,
    };
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
        return {
          conn_type: kind,
          name: connectionName,
          save_password: savePassword,
          ...x,
        };
      default:
        message("pass wrong kind of parameter, kind:" + kind, {
          kind: "error",
        });
    }
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        label="database"
        setName={setDatabase}
        makeConfig={makeConfig}
      />
      <MysqlSSl ssl={ssl} setSsl={setSsl} />
    </Stack>
  );
}

function Oracle(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 1521) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(config?.db);
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "oracle",
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database,
      schema: schema ? schema : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack>
      <Typography
        color="info"
        style={{ cursor: "pointer" }}
        onClick={() => {
          openUrl(
            "https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html",
          );
        }}
      >
        oracle need extra driver
      </Typography>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <TextField
        value={database}
        label="database"
        onChange={e => {
          setDatabase(e.target.value);
        }}
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          makeConfig={makeConfig}
          label="schema"
        />
      )}
    </Stack>
  );
}

function ClickHouse(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 8123) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [https, setHttps] = useState(config?.https);
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "clickhouse",
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      save_password: savePassword,
      db: database ? database : null,
      https: !!https,
      read_only: readOnly,
      conn_timeout: connTimeOut,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      />
      <Https https={https} setHttps={setHttps} />
    </Stack>
  );
}

function PostGres(props: { config; kind: DataSourceKind }) {
  const { config, kind } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState<string>((config?.port || 5432) + "");

  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [ssl, setSSl] = useState<types.PgSsl>({
    ssl_mode: config?.ssl_mode || "prefer",
    ssl_ca: config?.ssl_ca,
    ssl_cert: config?.ssl_cert,
    ssl_key: config?.ssl_key,
  });
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  const [savePassword, setSavePassword] = useState(getSavePassword(config));

  function makeConfig(): types.DBConnectionConfig {
    const x: types.PGDBConfig = {
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      db: database ? database : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      schema: schema ? schema : null,
      ...ssl,
    };
    switch (kind) {
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        return {
          conn_type: kind,
          name: connectionName,
          save_password: savePassword,
          ...x,
        };
      default:
        message("pass wrong kind of parameter, kind:" + kind, {
          kind: "error",
        });
    }
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        makeConfig={makeConfig}
        label="database"
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <PGSSL ssl={ssl} setSsl={setSSl} />
    </Stack>
  );
}

function Sqlite(props: { config? }) {
  const { config } = props;
  const [path, setPath] = useState(config?.path);
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "sqlite",
      name: connectionName,
      path,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <SelectPath path={path} setPath={setPath} required label="sqlite database file" />
    </Stack>
  );
}

const AddConnectionContext = createContext<{
  connectionName: string;
  readOnly: boolean;
  connTimeOut: number | null;
  connectionNameDom: RefObject<HTMLInputElement>;
  handleClose: () => void;
}>(null);

// global var make it simple the pass the parameter
// add connection run in separete window
let globalMakeConfig: MakeConfig = null;

function TestConnAndSave(props: { connectionNameDom }) {
  const { connectionNameDom } = props;
  const [testLoading, setTestLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleTestConnection = async () => {
    const config = globalMakeConfig();
    if (!config.name) {
      config.name = "test_connection";
    }
    if (config) {
      console.info("test config:", config);
      setTestLoading(true);
      invoke("test_connection", { config })
        .then(() => {
          showNotification("success", "test connection ok");
        })
        .catch(e => {
          showNotification("warning", "test connection failed", e);
        })
        .finally(() => {
          setTestLoading(false);
        });
    }
  };

  const handleSaveConnection = async () => {
    const config = globalMakeConfig();
    if (!config) {
      return;
    }
    if (!config.name) {
      connectionNameDom.current.focus();
      showNotification("warning", "connection name is empty.");
      return;
    }
    const password = config["password"];
    const savePassword = config["save_password"];
    if (config.name.includes("/")) {
      showNotification("error", `connection name can't  contains '/'`);
      connectionNameDom.current.focus();
      return;
    }
    if (password) {
      try {
        PassWordManager.savePassword(config.name, password, savePassword);
      } catch (e) {
        showNotification("error", "save password failed", e);
        return;
      }
    }
    if (password) {
      config["password"] = "";
    }
    ConnectionHandler.save(config)
      .then(() => {
        showNotification("success", "add connection OK");
        emit(connectionChangeEvent, "");
        // handleClose();
      })
      .catch(e => {
        showNotification("error", "add connection failure", e);
      });
  };
  return (
    <Stack>
      <Divider />
      <StackRowCenter>
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
      </StackRowCenter>
    </Stack>
  );
}

function getHost(config?: { host: string }) {
  return config?.host ? config.host : "localhost";
}

function getUser(config?: { username: string }) {
  return config?.username ? config.username : "";
}
function getPassword(config?: { password: string }) {
  return config?.password ? config.password : "";
}
function getSavePassword(config?: { save_password: boolean }) {
  return config ? config.save_password : true;
}

function getDB(config?: { db: string }) {
  return config ? config.db : "";
}
function getSchema(config?: { schema: string }) {
  return config ? config.schema : "";
}

function FireBird(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 3050) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  const [database, setDatabase] = useState(getDB(config));
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "firebird",
      name: connectionName,
      host,
      port: parseInt(port),
      username: user,
      password,
      save_password: savePassword,
      db: database ? database : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <TextField
        label="database"
        value={database}
        onChange={e => {
          setDatabase(e.target.value);
        }}
      />
    </Stack>
  );
}

function Redis(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 6379) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config) + "");
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const { connectionName, connTimeOut, readOnly } = useContext(AddConnectionContext);
  const [enableSSL, setEnableSSL] = useState(!!config?.enable_ssl);
  function makeConfig(): types.DBConnectionConfig {
    return {
      name: connectionName,
      conn_type: "redis",
      host,
      port: parseInt(port),
      username: user,
      password: password ? password : null,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      db: database ? database : null,
      save_password: savePassword,
      enable_ssl: enableSSL,
    };
  }

  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
      <FormControlLabel
        control={
          <Checkbox
            checked={enableSSL}
            onChange={e => setEnableSSL(e.target.checked)}
          />
        }
        label="enable ssl"
      />
    </Stack>
  );
}

function MongoDb(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 27017) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [enableSSL, setEnableSSL] = useState(false);
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);

  const [sslFiles, setSslFile] = useState<types.SSLFiles>({
    ssl_ca: null,
    ssl_cert: null,
    ssl_key: null,
  });
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "mongo",
      name: connectionName,
      read_only: readOnly,
      host,
      username: user ? user : null,
      password: password ? password : null,
      conn_timeout: connTimeOut,
      port: parseInt(port),
      save_password: savePassword,
      enable_ssl: enableSSL,
      db: database ? database : null,
      ...sslFiles,
    };
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
      <Stack direction="row" alignItems="center">
        <FormControlLabel
          style={{ width: "180px" }}
          control={
            <Checkbox
              checked={enableSSL}
              onChange={e => setEnableSSL(e.target.checked)}
            />
          }
          label="Enable SSL"
        />
        <SslFiles enabled={enableSSL} files={sslFiles} setFiles={setSslFile} />
      </Stack>
    </Stack>
  );
}

function Cassandra(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 9042) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const [database, setDatabase] = useState("");
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "cassandra",
      name: connectionName,
      host,
      username: user ? user : "",
      password: password ? password : null,
      db: database ? database : null,
      save_password: savePassword,
      read_only: readOnly,
      conn_timeout: connTimeOut,
      port: parseInt(port),
      ssl: { ssl_ca: null, ssl_cert: null, ssl_key: null },
    };
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
    </Stack>
  );
}

function SqlServer(props: { config? }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState((config?.port || 1433) + "");
  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [savePassword, setSavePassword] = useState(getSavePassword(config));

  const [database, setDatabase] = useState(getDB(config));
  const [sqlserverCert, setSqlServerCert] = useState<SqlServerCertData>({
    trust_cert: config ? config.trust_cert : false,
    cert_ca: config ? config.cert_ca : "",
  });
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "sqlserver",
      read_only: readOnly,
      conn_timeout: connTimeOut,
      username: user,
      password: password,
      name: connectionName,
      host,
      db: database ? database : null,
      port: parseInt(port),
      save_password: savePassword,
      trust_cert: sqlserverCert.trust_cert,
      cert_ca: sqlserverCert.cert_ca ? sqlserverCert.cert_ca : null,
      schema: schema ? schema : null,
    };
  }
  globalMakeConfig = makeConfig;

  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <DbSchemaCadidates
        name={database}
        setName={setDatabase}
        label="database"
        makeConfig={makeConfig}
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <SqlServerCert data={sqlserverCert} setData={setSqlServerCert} />
    </Stack>
  );
}

function AddConnection(props: {
  handleClose: () => void;
  config?: types.DBConnectionConfig;
}) {
  const { handleClose, config } = props;
  const [connectionType, setConnectionType] = useState(config?.conn_type || "mysql");
  const [connectionName, setConnectionName] = useState(config?.name);
  const [connTimeout, setConnTimeout] = useState<string>(
    config && config.conn_timeout ? config.conn_timeout.toString() : "",
  );
  const connectionNameDom = useRef<HTMLInputElement>(null);
  const [readOnly, setReadOnly] = useState(config?.read_only);
  const [cnameDuplicated, setCnameDuplicated] = useState(false);
  useEffect(() => {
    ConnectionHandler.nameDuplicated(connectionName).then(setCnameDuplicated);
  }, [connectionName]);

  return (
    <Stack>
      <FormControl fullWidth>
        <InputLabel>Connection Type</InputLabel>
        <Select
          value={connectionType}
          label="Connection Type"
          onChange={e => setConnectionType(e.target.value as DataSourceKind)}
          disabled={!!config}
        >
          <MenuItem value={DataSourceKind.Mysql}>MySQL</MenuItem>
          <MenuItem value={DataSourceKind.MariaDB}>MariaDB</MenuItem>
          <MenuItem value={DataSourceKind.TiDB}>TIDB</MenuItem>
          <MenuItem value={DataSourceKind.Postgres}>PostgreSQL</MenuItem>
          <MenuItem value={DataSourceKind.Sqlite}>SQLite</MenuItem>
          <MenuItem value={DataSourceKind.Redis}>Redis</MenuItem>
          <MenuItem value={DataSourceKind.Mongo}>Mongo</MenuItem>
          <MenuItem value={DataSourceKind.Oracle}>Oracle</MenuItem>
          <MenuItem value={DataSourceKind.SqlServer}>Sql Server(microsoft)</MenuItem>
          <MenuItem value={DataSourceKind.Cassandra}>Cassandra</MenuItem>
          <MenuItem value={DataSourceKind.ClickHouse}>Clickhouse</MenuItem>
          <MenuItem value={DataSourceKind.LibSql}> LibSql </MenuItem>
          <MenuItem value={DataSourceKind.FireBird}> FireBird </MenuItem>
          <MenuItem value={DataSourceKind.DuckDB}> Duckdb </MenuItem>
          <MenuItem value={DataSourceKind.Cockroach}> Cockroach </MenuItem>
          {/* <MenuItem value={DataSourceKind.Db2}> Db2(luw) </MenuItem> */}
        </Select>
      </FormControl>
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <TextField
          label="Connection Name"
          value={connectionName}
          inputRef={connectionNameDom}
          required
          onChange={e => setConnectionName(e.target.value)}
          error={cnameDuplicated}
          helperText={cnameDuplicated ? "Connection name duplicated" : ""}
          slotProps={{
            input: {
              endAdornment: connectionName && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnectionName("");
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
          onChange={e => setConnTimeout(e.target.value)}
          slotProps={{
            input: {
              endAdornment: connTimeout && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setConnTimeout("");
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
          style={{ width: "180px" }}
          control={
            <Checkbox
              checked={readOnly}
              onChange={e => setReadOnly(e.target.checked)}
            />
          }
          label="Read Only"
        />
      </Stack>

      <AddConnectionContext
        value={{
          connectionName,
          connectionNameDom,
          handleClose,
          readOnly: !!readOnly,
          connTimeOut: connTimeout ? parseInt(connTimeout) : null,
        }}
      >
        <Stack>
          <Typography variant="h6"> server info </Typography>
          <Divider />
          {(connectionType === DataSourceKind.Mysql ||
            connectionType === DataSourceKind.MariaDB ||
            connectionType === DataSourceKind.TiDB) && (
            <Mysql config={config} kind={connectionType as DataSourceKind} />
          )}

          {connectionType === DataSourceKind.LibSql && <LibSql config={config} />}

          {connectionType === DataSourceKind.FireBird && <FireBird config={config} />}

          {(connectionType === DataSourceKind.Postgres ||
            connectionType === DataSourceKind.Cockroach) && (
            <PostGres config={config} kind={connectionType as DataSourceKind} />
          )}
          {connectionType === DataSourceKind.ClickHouse && (
            <ClickHouse config={config} />
          )}

          {connectionType === DataSourceKind.Cassandra && <Cassandra config={config} />}

          {connectionType === DataSourceKind.Redis && <Redis config={config} />}
          {connectionType === DataSourceKind.Mongo && <MongoDb config={config} />}
          {connectionType === DataSourceKind.SqlServer && <SqlServer config={config} />}
          {connectionType === DataSourceKind.Oracle && <Oracle config={config} />}
          {connectionType === DataSourceKind.Sqlite && <Sqlite config={config} />}
          {connectionType === DataSourceKind.DuckDB && <DuckDB config={config} />}

          {connectionType === DataSourceKind.Db2 && <Db2 config={config} />}
        </Stack>
      </AddConnectionContext>

      <TestConnAndSave connectionNameDom={connectionNameDom} />
    </Stack>
  );
}

function DuckDB(props: { config? }) {
  const { config } = props;

  function getConfigStr(key: string) {
    if (!config) {
      return "";
    }
    const v = config[key];
    return v ? v : "";
  }
  const [path, setPath] = useState(getConfigStr("path"));
  const [db, setDb] = useState(getConfigStr("db"));
  const [schema, setSchema] = useState(getConfigStr("schema"));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  function makeConfig(): types.DBConnectionConfig {
    const x: types.DBConnectionConfig = {
      conn_type: "duckdb",
      path,
      db: db ? db : null,
      conn_timeout: connTimeOut,
      name: connectionName,
      read_only: readOnly,
      schema: schema ? schema : null,
    };
    console.info("makeConfig", x);
    return x;
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack>
      <SelectPath path={path} setPath={setPath} label="database file path" required />
      {path && (
        <DbSchemaCadidates
          name={db}
          setName={setDb}
          makeConfig={makeConfig}
          label="database"
        />
      )}
      {db && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
    </Stack>
  );
}

function LibSql(props: { config }) {
  const { config } = props;
  const urlOld = config && config.source == "remote" && config.url;
  const [url, setUrl] = useState(urlOld ? urlOld : "");
  // const [replica, setReplica] = useState(config?.replica);
  const oldAuth = config && config.source == "remote" && config.auth_token;
  const [authToken, setAuthToken] = useState(oldAuth ? oldAuth : "");
  const [libSqlModel] = useState<"remote">(config?.source || "remote");
  const [path] = useState(config?.path);
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);

  function makeConfig(): types.DBConnectionConfig {
    const x: types.DBConnectionConfig = {
      conn_type: "libsql",
      name: connectionName,
      conn_timeout: connTimeOut,
      source: libSqlModel,
      url: url,
      auth_token: authToken,
      replica: false,
      read_only: readOnly,
      path: path ? path : null,
    };
    console.info("makeConfig", x);
    return x;
  }

  globalMakeConfig = makeConfig;

  // const models = ['remote', 'local'];
  return (
    <>
      {/* <RadioGroup
        row
        onChange={(e) => {
          setLibSqlModal(e.target.value as 'remote');
        }}
        value={libSqlModel}
      >
        {models.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup> */}

      {/* {libSqlModel === "local" && (
        <SelectPath
          path={path}
          setPath={setPath}
          required
          label="libsql database file"
        />
      )} */}
      {libSqlModel === "remote" && (
        <>
          <Stack direction="row">
            <TextField
              value={url}
              label="url"
              onChange={e => {
                setUrl(e.target.value);
              }}
              fullWidth
              slotProps={{
                input: {
                  endAdornment: url && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setUrl("");
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
              value={authToken}
              label="auth token "
              onChange={e => {
                setAuthToken(e.target.value);
              }}
              slotProps={{
                input: {
                  endAdornment: authToken && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setAuthToken("");
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
          {/* <Stack
            direction="row"
            style={{ alignItems: 'center' }}
            
            flex={1}
          >
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
          </Stack> */}
        </>
      )}
    </>
  );
}

function OdbcDriverCommon(props: { config: RefObject<types.OdbcCommon> }) {
  const { config } = props;
  const [driverName, setDriverName] = useState(config.current.driver_name);
  const [maxStrLen, setMaxStrLen] = useState(config.current.max_str_len + "");
  const [maxbinLen, setMaxBinLen] = useState(config.current.max_bin_len + "");

  useEffect(() => {
    // eslint-disable-next-line react-hooks/immutability
    config.current.driver_name = driverName;
    config.current.max_str_len = parseInt(maxStrLen);
    config.current.max_bin_len = parseInt(maxbinLen);
  }, [config, driverName, maxStrLen, maxbinLen]);

  return (
    <Stack direction="row">
      <TextField
        label="driver name"
        value={driverName}
        sx={{ mr: 1 }}
        onChange={e => setDriverName(e.target.value)}
        slotProps={{
          input: {
            endAdornment: driverName && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setDriverName("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        required
      />

      <TextField
        label="max string length"
        value={maxStrLen}
        sx={{ mr: 1 }}
        onChange={e => setMaxStrLen(e.target.value)}
        slotProps={{
          input: {
            endAdornment: maxStrLen && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setMaxStrLen("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        error={maxStrLen ? isNaN(parseInt(maxStrLen)) : false}
        required
      />

      <TextField
        label="max bin length"
        value={maxbinLen}
        sx={{ mr: 1 }}
        onChange={e => setMaxBinLen(e.target.value)}
        slotProps={{
          input: {
            endAdornment: maxbinLen && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setMaxBinLen("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        error={maxbinLen ? isNaN(parseInt(maxbinLen)) : false}
        required
      />
    </Stack>
  );
}

//@ts-ignore
// eslint-disable-next-line @typescript-eslint/no-unused-vars
function Db2(props: { config?: any }) {
  const { config } = props;
  const [host, setHost] = useState(getHost(config));
  const [port, setPort] = useState<string>((config?.port || 50000) + "");

  const [user, setUser] = useState(getUser(config));
  const [password, setPassword] = useState(getPassword(config));
  const [database, setDatabase] = useState(getDB(config));
  const [schema, setSchema] = useState(getSchema(config));
  const { connTimeOut, connectionName, readOnly } = useContext(AddConnectionContext);
  const [savePassword, setSavePassword] = useState(getSavePassword(config));
  const odbccommon = useRef<types.OdbcCommon>({
    driver_name: "Driver_IBM_DB2",
    max_str_len: 4096,
    max_bin_len: 4096,
  });

  function makeConfig(): types.DBConnectionConfig {
    return {
      conn_type: "db2",
      host,
      port: parseInt(port),
      username: user,
      password: password,
      save_password: savePassword,
      db: database,
      schema: schema ? schema : null,
      conn_timeout: connTimeOut,
      read_only: readOnly,
      name: connectionName,
      ...odbccommon.current,
    };
  }
  globalMakeConfig = makeConfig;
  return (
    <Stack>
      <HostAndPort host={host} port={port} setHost={setHost} setPort={setPort} />
      <UserAndPassword
        user={user}
        password={password}
        setUser={setUser}
        setPassword={setPassword}
        savePassword={savePassword}
        setSavePassword={setSavePassword}
      />
      <TextField
        label="database"
        value={database}
        sx={{ mr: 1 }}
        onChange={e => setDatabase(e.target.value)}
        slotProps={{
          input: {
            endAdornment: database && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setDatabase("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        required
      />
      {database && (
        <DbSchemaCadidates
          name={schema}
          setName={setSchema}
          label="schema"
          makeConfig={makeConfig}
        />
      )}
      <OdbcDriverCommon config={odbccommon} />
    </Stack>
  );
}

export { AddConnection };
