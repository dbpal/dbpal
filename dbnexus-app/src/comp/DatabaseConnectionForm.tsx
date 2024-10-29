import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  TextField,
  Button,
  Switch,
  FormGroup,
  FormControlLabel,
  Typography,
  Box,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Paper,
} from '@mui/material';

export default function DatabaseConnectionForm() {
  const navigate = useNavigate(); // 添加这行来使用 navigate 函数

  const [connectionType, setConnectionType] = useState('MySQL');
  const [connectionMode, setConnectionMode] = useState('Host and Port');
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('3306');
  const [user, setUser] = useState('');
  const [password, setPassword] = useState('');
  const [defaultDatabase, setDefaultDatabase] = useState('');
  const [useSSL, setUseSSL] = useState(false);
  const [connectionName, setConnectionName] = useState('');

  const handleConnect = () => {
    // 模拟连接数据库
    alert('Connecting to database...');
    navigate('/database'); // 在连接逻辑之后添加跳转
  };

  const handleSaveConnection = () => {
    // 模拟保存连接
    alert(`Connection ${connectionName} saved.`);
    navigate('/database'); // 这里也可以添加跳转，视业务逻辑而定
  };

  const handleTestConnection = () => {
    // 模拟测试连接
    alert('Testing connection...');
  };

  return (
    <Paper sx={{ maxWidth: 600, m: 2, p: 2, bgcolor: 'background.paper' }}>
      <Typography variant='h6' gutterBottom>
        New Connection
      </Typography>
      <FormControl fullWidth margin='normal'>
        <InputLabel>Connection Type</InputLabel>
        <Select
          value={connectionType}
          label='Connection Type'
          onChange={(e) => setConnectionType(e.target.value)}
        >
          <MenuItem value='MySQL'>MySQL</MenuItem>
          <MenuItem value='PostgreSQL'>PostgreSQL</MenuItem>
          <MenuItem value='SQLite'>SQLite</MenuItem>
        </Select>
      </FormControl>
      <FormControl fullWidth margin='normal'>
        <InputLabel>Connection Mode</InputLabel>
        <Select
          value={connectionMode}
          label='Connection Mode'
          onChange={(e) => setConnectionMode(e.target.value)}
        >
          <MenuItem value='Host and Port'>Host and Port</MenuItem>
          <MenuItem value='Socket'>Socket</MenuItem>
        </Select>
      </FormControl>

      <Box sx={{ mt: 2, display: 'flex', justifyContent: 'left' }}>
        <TextField
          margin='normal'
          fullWidth
          label='Host'
          value={host}
          sx={{ mr: 1 }}
          onChange={(e) => setHost(e.target.value)}
        />
        <TextField
          margin='normal'
          fullWidth
          label='Port'
          value={port}
          onChange={(e) => setPort(e.target.value)}
        />
      </Box>

      <FormGroup>
        <FormControlLabel
          control={
            <Switch
              checked={useSSL}
              onChange={(e) => setUseSSL(e.target.checked)}
            />
          }
          label='Enable SSL'
        />
      </FormGroup>

      <Box sx={{ mt: 2, display: 'flex', justifyContent: 'left' }}>
        <TextField
          margin='normal'
          fullWidth
          label='User'
          value={user}
          sx={{ mr: 1 }}
          onChange={(e) => setUser(e.target.value)}
        />
        <TextField
          margin='normal'
          fullWidth
          label='Password'
          type='password'
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Box>

      <TextField
        margin='normal'
        fullWidth
        label='Default Database'
        value={defaultDatabase}
        onChange={(e) => setDefaultDatabase(e.target.value)}
      />
      <Box sx={{ mt: 2, display: 'flex', justifyContent: 'left' }}>
        <Button
          variant='outlined'
          sx={{ mr: 1 }}
          onClick={handleTestConnection}
        >
          Test
        </Button>
        <Button variant='contained' onClick={handleConnect}>
          Connect
        </Button>
      </Box>
      <TextField
        margin='normal'
        fullWidth
        label='Connection Name'
        value={connectionName}
        onChange={(e) => setConnectionName(e.target.value)}
        helperText='Enter a name for this connection'
      />

      <Button
        variant='contained'
        color='primary'
        sx={{ mt: 2, width: '100%' }}
        onClick={handleSaveConnection}
      >
        Save
      </Button>
    </Paper>
  );
}
