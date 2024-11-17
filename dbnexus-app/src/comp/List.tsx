import * as React from 'react';
import ListItemButton from '@mui/material/ListItemButton';
import ListItemIcon from '@mui/material/ListItemIcon';
import ListItemText from '@mui/material/ListItemText';

import ShoppingCartIcon from '@mui/icons-material/ShoppingCart';
import PeopleIcon from '@mui/icons-material/People';
import BarChartIcon from '@mui/icons-material/BarChart';
import LayersIcon from '@mui/icons-material/Layers';

import { Button, Grid, Typography } from '@mui/material';
import { useState } from 'react';
import { secondConfirm } from './Confirm';
import BrowserNotSupported from '@mui/icons-material/BrowserNotSupported';

function MainListItems() {
  const [anchorEl, setAnchorEl] = React.useState(null);
  const open = Boolean(anchorEl);

  const handleClick = (event: any) => {
    setAnchorEl(event.currentTarget);
  };

  const handleClose = () => {
    setAnchorEl(null);
  };

  return (
    <React.Fragment>
      <ListItemButton>
        <ListItemIcon>
          <ShoppingCartIcon />
        </ListItemIcon>
        <ListItemText primary='Edit' />
      </ListItemButton>
      <ListItemButton>
        <ListItemIcon>
          <PeopleIcon />
        </ListItemIcon>
        <ListItemText primary='View' />
      </ListItemButton>
      <ListItemButton>
        <ListItemIcon>
          <BarChartIcon />
        </ListItemIcon>
        <ListItemText primary='Tools' />
      </ListItemButton>
      <ListItemButton>
        <ListItemIcon>
          <LayersIcon />
        </ListItemIcon>
        <ListItemText primary='Help' />
      </ListItemButton>
    </React.Fragment>
  );
}

export function ConnectList() {
  const [isConnected, setIsConnected] = useState(false);
  const [connectionURL, setConnectionURL] = useState('');

  const handleDelete = async () => {
    const result = await secondConfirm({
      title: 'Delete Item',
      content: 'Are you sure you want to delete this item?',
      confirmText: 'Delete',
      cancelText: 'Cancel',
    });

    if (result) {
      // TODO: Delete
      console.log('Item deleted');
    }
  };

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

  // 管理数据库连接状态
  return (
    <React.Fragment>
      {isConnected ? (
        <>
          <Typography variant='body1' sx={{ marginRight: 2, ml: 2 }}>
            Connected to: {connectionURL}
          </Typography>
          <Button variant='contained' color='error' onClick={handleDisconnect}>
            Disconnect
          </Button>
        </>
      ) : (
        <Button
          sx={{ mx: 4, mt: 2, mb: 2 }}
          color='inherit'
          variant='outlined'
          onClick={handleConnect}
        >
          Connect
        </Button>
      )}

      <ListItemButton sx={{ pl: 4 }}>
        <Grid>
          <ListItemText primary='MySQL' />
          <ListItemText secondary='localhost:3660' />
        </Grid>
        <ListItemIcon sx={{ ml: 4, my: 0 }}>
          <BrowserNotSupported />
        </ListItemIcon>
      </ListItemButton>

      <ListItemButton sx={{ pl: 4 }}>
        <Grid>
          <ListItemText primary='MySQL' />
          <ListItemText secondary='localhost:3660' />
        </Grid>
        <ListItemIcon sx={{ ml: 4, my: 0 }}>
          <BrowserNotSupported />
        </ListItemIcon>
      </ListItemButton>

      <ListItemButton sx={{ pl: 4 }}>
        {/* <ListItemIcon>
        <Airplay />
      </ListItemIcon> */}
        <Grid>
          <ListItemText primary='MySQL' />
          <ListItemText secondary='localhost:3660' />
        </Grid>
        <ListItemIcon sx={{ ml: 4, my: 0 }} onClick={handleDelete}>
          <BrowserNotSupported />
        </ListItemIcon>
      </ListItemButton>
    </React.Fragment>
  );
}

export default MainListItems;
