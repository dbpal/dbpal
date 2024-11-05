import { useState } from 'react';
import Toolbar from '@mui/material/Toolbar';
import IconButton from '@mui/material/IconButton';
import Button from '@mui/material/Button';
import Typography from '@mui/material/Typography';
import MenuIcon from '@mui/icons-material/Menu';
import { Menu, MenuItem } from '@mui/material';
import React from 'react';

function NavBar() {
  const [fileMenuAnchorEl, setFileMenuAnchorEl] = useState<null | HTMLElement>(null);
  const [editMenuAnchorEl, setEditMenuAnchorEl] = useState<null | HTMLElement>(null);
  const [toolsMenuAnchorEl, setToolsMenuAnchorEl] = useState<null | HTMLElement>(null);
  const [helpMenuAnchorEl, setHelpMenuAnchorEl] = useState<null | HTMLElement>(null);

  const openFileMenu = Boolean(fileMenuAnchorEl);
  const openEditMenu = Boolean(editMenuAnchorEl);
  const openToolsMenu = Boolean(toolsMenuAnchorEl);
  const openHelpMenu = Boolean(helpMenuAnchorEl);

  const handleMenuClick = (
    event: React.MouseEvent<HTMLButtonElement>,
    setAnchorEl: React.Dispatch<React.SetStateAction<HTMLElement | null>>
  ) => {
    setAnchorEl(event.currentTarget);
  };

  const handleMenuClose = (
    setAnchorEl: React.Dispatch<React.SetStateAction<HTMLElement | null>>
  ) => {
    setAnchorEl(null);
  };

  // 管理数据库连接状态
  const [isConnected, setIsConnected] = useState(false);
  const [connectionURL, setConnectionURL] = useState('');

  // 模拟连接数据库
  const handleConnect = () => {
    setIsConnected(true);
    setConnectionURL('localhost:3304');
  };

  // 模拟断开数据库连接
  const handleDisconnect = () => {
    setIsConnected(false);
    setConnectionURL('');
  };

  return (
    <>
      {/* <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-file'
        aria-controls={openFileMenu ? 'nav-file-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={openFileMenu ? 'true' : undefined}
        onClick={(e) => handleMenuClick(e, setFileMenuAnchorEl)}
      >
        File
      </Button>
      <Menu
        id='nav-file-menu'
        anchorEl={fileMenuAnchorEl}
        open={openFileMenu}
        onClose={() => handleMenuClose(setFileMenuAnchorEl)}
      >
        <MenuItem onClick={() => handleMenuClose(setFileMenuAnchorEl)}>Open</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setFileMenuAnchorEl)}>Close</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setFileMenuAnchorEl)}>Exit</MenuItem>
      </Menu>

      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-edit'
        aria-controls={openEditMenu ? 'nav-edit-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={openEditMenu ? 'true' : undefined}
        onClick={(e) => handleMenuClick(e, setEditMenuAnchorEl)}
      >
        Edit
      </Button>
      <Menu
        id='nav-edit-menu'
        anchorEl={editMenuAnchorEl}
        open={openEditMenu}
        onClose={() => handleMenuClose(setEditMenuAnchorEl)}
      >
        <MenuItem onClick={() => handleMenuClose(setEditMenuAnchorEl)}>Undo</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setEditMenuAnchorEl)}>Redo</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setEditMenuAnchorEl)}>Copy</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setEditMenuAnchorEl)}>Paste</MenuItem>
      </Menu>

      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-tools'
        aria-controls={openToolsMenu ? 'nav-tools-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={openToolsMenu ? 'true' : undefined}
        onClick={(e) => handleMenuClick(e, setToolsMenuAnchorEl)}
      >
        Tools
      </Button>
      <Menu
        id='nav-tools-menu'
        anchorEl={toolsMenuAnchorEl}
        open={openToolsMenu}
        onClose={() => handleMenuClose(setToolsMenuAnchorEl)}
      >
        <MenuItem onClick={() => handleMenuClose(setToolsMenuAnchorEl)}>Data Import</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setToolsMenuAnchorEl)}>Data Export</MenuItem>
      </Menu>

      <Button
        color='inherit'
        sx={{ marginRight: 2 }}
        id='nav-b-help'
        aria-controls={openHelpMenu ? 'nav-help-menu' : undefined}
        aria-haspopup='true'
        aria-expanded={openHelpMenu ? 'true' : undefined}
        onClick={(e) => handleMenuClick(e, setHelpMenuAnchorEl)}
      >
        Help
      </Button>
      <Menu
        id='nav-help-menu'
        anchorEl={helpMenuAnchorEl}
        open={openHelpMenu}
        onClose={() => handleMenuClose(setHelpMenuAnchorEl)}
      >
        <MenuItem onClick={() => handleMenuClose(setHelpMenuAnchorEl)}>Check Update</MenuItem>
        <MenuItem onClick={() => handleMenuClose(setHelpMenuAnchorEl)}>Visit Website</MenuItem>
      </Menu> */}

      {/* Connection Status */}
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
