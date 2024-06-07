import { useState } from 'react';
import Toolbar from '@mui/material/Toolbar';
import IconButton from '@mui/material/IconButton';
import Button from '@mui/material/Button';
import Typography from '@mui/material/Typography';
import MenuIcon from '@mui/icons-material/Menu';


function NavBar() {
    // 管理数据库连接状态
    const [isConnected, setIsConnected] = useState(false);
    const [connectionURL, setConnectionURL] = useState('');

    // 模拟连接数据库
    const handleConnect = () => {
        // 这里可以替换成实际的数据库连接逻辑
        setIsConnected(true);
        setConnectionURL('localhost:3304');  // 假设连接到这个端口
    };

    // 模拟断开数据库连接
    const handleDisconnect = () => {
        setIsConnected(false);
        setConnectionURL('');
    };

    return (
        <>
            {isConnected ? (
                <>
                    <Typography variant="body1" sx={{ marginRight: 2 }}>
                        Connected to: {connectionURL}
                    </Typography>
                    <Button variant="contained" color="error" onClick={handleDisconnect}>
                        Disconnect
                    </Button>
                </>
            ) : (
                <Button color="inherit" variant="outlined" onClick={handleConnect}>
                    Connect
                </Button>
            )}
        </>
    );
}

export default NavBar;