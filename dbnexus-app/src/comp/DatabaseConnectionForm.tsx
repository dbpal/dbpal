import React, { useState } from 'react';
import { TextField, Button, Switch, FormGroup, FormControlLabel, Typography, Box, FormControl, InputLabel, Select, MenuItem } from '@mui/material';

export default function DatabaseConnectionForm() {
    const [connectionType, setConnectionType] = useState('MySQL');
    const [connectionMode, setConnectionMode] = useState('Host and Port');
    const [host, setHost] = useState('localhost');
    const [port, setPort] = useState('3306');
    const [user, setUser] = useState('');
    const [password, setPassword] = useState('');
    const [defaultDatabase, setDefaultDatabase] = useState('');
    const [useSSL, setUseSSL] = useState(false);

    const handleTestConnection = () => {
        // 模拟测试连接
        alert('Testing connection...');
    };

    const handleSaveConnection = () => {
        // 模拟保存连接
        alert('Connection saved.');
    };

    return (
        <Box sx={{ maxWidth: 400, m: 'auto', p: 4, bgcolor: 'background.default', mt: 2 }}>
            <Typography variant="h6" gutterBottom>
                New Connection
            </Typography>
            <FormControl fullWidth margin="normal">
                <InputLabel>Connection Type</InputLabel>
                <Select
                    value={connectionType}
                    label="Connection Type"
                    onChange={(e) => setConnectionType(e.target.value)}
                >
                    <MenuItem value="MySQL">MySQL</MenuItem>
                    <MenuItem value="PostgreSQL">PostgreSQL</MenuItem>
                    <MenuItem value="SQLite">SQLite</MenuItem>
                </Select>
            </FormControl>
            <FormControl fullWidth margin="normal">
                <InputLabel>Connection Mode</InputLabel>
                <Select
                    value={connectionMode}
                    label="Connection Mode"
                    onChange={(e) => setConnectionMode(e.target.value)}
                >
                    <MenuItem value="Host and Port">Host and Port</MenuItem>
                    <MenuItem value="Socket">Socket</MenuItem>
                </Select>
            </FormControl>
            <TextField
                margin="normal"
                fullWidth
                label="Host"
                value={host}
                onChange={(e) => setHost(e.target.value)}
            />
            <TextField
                margin="normal"
                fullWidth
                label="Port"
                value={port}
                onChange={(e) => setPort(e.target.value)}
            />
            <FormGroup>
                <FormControlLabel
                    control={<Switch checked={useSSL} onChange={(e) => setUseSSL(e.target.checked)} />}
                    label="Enable SSL"
                />
            </FormGroup>
            <TextField
                margin="normal"
                fullWidth
                label="User"
                value={user}
                onChange={(e) => setUser(e.target.value)}
            />
            <TextField
                margin="normal"
                fullWidth
                label="Password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
            />
            <TextField
                margin="normal"
                fullWidth
                label="Default Database"
                value={defaultDatabase}
                onChange={(e) => setDefaultDatabase(e.target.value)}
            />
            <Box sx={{ mt: 2, display: 'flex', justifyContent: 'space-between' }}>
                <Button variant="outlined" onClick={handleTestConnection}>Test</Button>
                <Button variant="contained" onClick={handleSaveConnection}>Connect</Button>
            </Box>
            <Button variant="contained" color="primary" sx={{ mt: 2, width: '100%' }} onClick={handleSaveConnection}>
                Save
            </Button>
        </Box>
    );
}