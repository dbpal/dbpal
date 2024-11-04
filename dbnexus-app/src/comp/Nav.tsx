import { useState } from 'react';
import Toolbar from '@mui/material/Toolbar';
import IconButton from '@mui/material/IconButton';
import Button from '@mui/material/Button';
import Typography from '@mui/material/Typography';
import MenuIcon from '@mui/icons-material/Menu';
import { Menu, MenuItem } from '@mui/material';
import React from 'react';

function NavBar() {
  const [anchorEl, setAnchorEl] = React.useState<null | HTMLElement>(null);
  const open = Boolean(anchorEl);
  const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
    setAnchorEl(event.currentTarget);
  };
  const handleClose = () => {
    setAnchorEl(null);
  };

  // 管理数据库连接状态
  const [isConnected, setIsConnected] = useState(false);
  const [connectionURL, setConnectionURL] = useState('');

  // 模拟连接数据库
  const handleConnect = () => {
    // 这里可以替换成实际的数据库连接逻辑
    setIsConnected(true);
    setConnectionURL('localhost:3304'); // 假设连接到这个端口
  };

  // 模拟断开数据库连接
  const handleDisconnect = () => {
    setIsConnected(false);
    setConnectionURL('');
  };

  return (
    <>
      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-file'
        aria-controls={open ? 'nav-file-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={open ? 'true' : undefined}
        onClick={handleClick}
      >
        File
      </Button>
      <Menu
        id='nav-file-menu'
        anchorEl={anchorEl}
        open={open}
        onClose={handleClose}
        MenuListProps={{
          'aria-labelledby': 'nav-b-file',
        }}
      >
        <MenuItem onClick={handleClose}>Open</MenuItem>
        <MenuItem onClick={handleClose}>Close</MenuItem>
        <MenuItem onClick={handleClose}>Exit</MenuItem>
      </Menu>
      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-edit'
        aria-controls={open ? 'nav-edit-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={open ? 'true' : undefined}
        onClick={handleClick}
      >
        Edit
      </Button>
      <Menu
        id='nav-edit-menu'
        anchorEl={anchorEl}
        open={open}
        onClose={handleClose}
        MenuListProps={{
          'aria-labelledby': 'nav-b-edit',
        }}
      >
        <MenuItem onClick={handleClose}>Undo</MenuItem>
        <MenuItem onClick={handleClose}>Redo</MenuItem>
        <MenuItem onClick={handleClose}>Copy</MenuItem>
        <MenuItem onClick={handleClose}>Paste</MenuItem>
      </Menu>
      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-tools'
        aria-controls={open ? 'nav-tools-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={open ? 'true' : undefined}
        onClick={handleClick}
      >
        Tools
      </Button>
      <Menu
        id='nav-tools-menu'
        anchorEl={anchorEl}
        open={open}
        onClose={handleClose}
        MenuListProps={{
          'aria-labelledby': 'nav-b-tools',
        }}
      >
        <MenuItem onClick={handleClose}>Data Import</MenuItem>
        <MenuItem onClick={handleClose}>Data Export</MenuItem>
      </Menu>

      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-help'
        aria-controls={open ? 'nav-help-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={open ? 'true' : undefined}
        onClick={handleClick}
      >
        Help
      </Button>
      <Menu
        id='nav-help-menu'
        anchorEl={anchorEl}
        open={open}
        onClose={handleClose}
        MenuListProps={{
          'aria-labelledby': 'nav-b-help',
        }}
      >
        <MenuItem onClick={handleClose}>Check Update</MenuItem>
        <MenuItem onClick={handleClose}>Visit Website</MenuItem>
      </Menu>
      {isConnected ? (
        <>
          <Typography variant='body1' sx={{ marginRight: 2 }}>
            Connected to: {connectionURL}
          </Typography>
          <Button variant='contained' color='error' onClick={handleDisconnect}>
            Disconnect
          </Button>
        </>
      ) : (
        <Button color='inherit' variant='outlined' onClick={handleConnect}>
          Connect
        </Button>
      )}
    </>
  );
}

export default NavBar;
