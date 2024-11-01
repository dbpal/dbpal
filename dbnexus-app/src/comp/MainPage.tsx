import * as React from 'react';
import { styled, createTheme, ThemeProvider } from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import MuiDrawer from '@mui/material/Drawer';
import Box from '@mui/material/Box';
import MuiAppBar, { AppBarProps as MuiAppBarProps } from '@mui/material/AppBar';
import Toolbar from '@mui/material/Toolbar';
import List from '@mui/material/List';
import Typography from '@mui/material/Typography';
import Divider from '@mui/material/Divider';
import IconButton from '@mui/material/IconButton';
import Badge from '@mui/material/Badge';
import Container from '@mui/material/Container';
import Grid from '@mui/material/Grid';
import Link from '@mui/material/Link';
import MenuIcon from '@mui/icons-material/Menu';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import NotificationsIcon from '@mui/icons-material/Notifications';
import MainListItems, { secondaryListItems } from './List';
import NavBar from './Nav';
import DatabaseConnectionForm from './DatabaseConnectionForm';
import { darkTheme } from './theme';
import { useState } from 'react';
import { Switch } from '@mui/material';
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import DatabaseTable from './DatabaseTable';
import { WidthFull } from '@mui/icons-material';
import DataList from './DataList';

const drawerWidth: number = 240;

interface AppBarProps extends MuiAppBarProps {
  open?: boolean;
}

const AppBar = styled(MuiAppBar, {
  shouldForwardProp: (prop) => prop !== 'open',
})(({ theme }) => ({
  zIndex: theme.zIndex.drawer + 1,
}));

const Drawer = styled(MuiDrawer, {
  shouldForwardProp: (prop) => prop !== 'open',
})(({ theme }) => ({
  '& .MuiDrawer-paper': {
    position: 'fixed',
    whiteSpace: 'nowrap',
    width: drawerWidth,
    transition: theme.transitions.create('width', {
      easing: theme.transitions.easing.sharp,
      duration: theme.transitions.duration.enteringScreen,
    }),
    boxSizing: 'border-box',
  },
}));

function MainWindow() {
  return (
    <Router>
      <Routes>
        <Route path='/' element={<DatabaseConnectionForm />} />
        <Route path='/database' element={<DatabaseTable />} />
      </Routes>
    </Router>
  );
}

export default function MainPage() {
  const [open, setOpen] = React.useState(true);
  const toggleDrawer = () => {
    setOpen(!open);
  };

  const renderDrawerContent = () => {
    if (location.pathname === '/database') {
      return { DataList };
    } else {
      return { secondaryListItems };
    }
  };

  const lightTheme = createTheme();

  const [theme, setTheme] = useState('light'); // 默认为明亮主题

  const isDarkMode = theme === 'dark';

  const toggleTheme = () => {
    setTheme((prevTheme) => (prevTheme === 'light' ? 'dark' : 'light'));
  };

  return (
    <ThemeProvider theme={theme === 'light' ? lightTheme : darkTheme}>
      <Box sx={{ display: 'flex' }}>
        <CssBaseline />

        <AppBar position='absolute'>
          <Toolbar
            sx={{
              pr: '25px', // keep right padding when drawer closed
            }}
          >
            <IconButton
              edge='start'
              color='inherit'
              aria-label='open drawer'
              onClick={toggleDrawer}
              sx={{
                marginRight: '36px',
                ...(open && { display: 'none' }),
              }}
            >
              <MenuIcon />
            </IconButton>

            <Typography
              component='h1'
              variant='h6'
              align='left'
              sx={{ width: '240px', ml: 1, mr: 3 }}
            >
              bdnexus
              <Switch
                checked={isDarkMode}
                onChange={toggleTheme}
                color='primary'
                sx={{
                  ml: 4,
                  '& .MuiSwitch-track': {
                    backgroundColor: isDarkMode ? 'grey.800' : 'grey.300',
                  },
                  '& .MuiSwitch-thumb': {
                    backgroundColor: isDarkMode ? 'yellow' : 'blue',
                  },
                }}
              />
            </Typography>

            <NavBar></NavBar>



            <Typography
              component='h1'
              variant='h6'
              color='inherit'
              noWrap
              sx={{ flexGrow: 1 }}
            ></Typography>

            <IconButton color='inherit'>
              <Badge badgeContent={4} color='secondary'>
                <NotificationsIcon />
              </Badge>
            </IconButton>
          </Toolbar>
        </AppBar>

        <Drawer sx={{ width: '240px' }} variant='permanent' open={open}>
          <Toolbar
            sx={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'flex-end',
              px: [1],
            }}
          >
            <IconButton onClick={toggleDrawer}>
              <ChevronLeftIcon />
            </IconButton>
          </Toolbar>

          <List component='nav'>
            {/* <MainListItems>
            </MainListItems> 
            <Divider sx={{ my: 1 }} />
            */}
            {location.pathname === '/database' ? (
              <DataList></DataList>
            ) : (
              secondaryListItems
            )}
          </List>
        </Drawer>

        <Box
          component='main'
          sx={{
            backgroundColor: (theme) =>
              theme.palette.mode === 'light'
                ? theme.palette.grey[100]
                : theme.palette.grey[900],
            flexGrow: 1,
            height: '100vh',
            overflow: 'auto',
          }}
        >
          <Toolbar />
          <Container maxWidth='lg' sx={{ mt: 4, mb: 4 }}>
            <Grid container spacing={3} sx={{ justifyContent: 'center' }}>
              <MainWindow></MainWindow>

              {/* Chart */}
              {/* <Grid item xs={12} md={8} lg={9}>
                <Paper
                  sx={{
                    p: 2,
                    display: 'flex',
                    flexDirection: 'column',
                    height: 240,
                  }}
                >
                  <Chart />
                </Paper>
              </Grid> */}
              {/* Recent Deposits */}
              {/* <Grid item xs={12} md={4} lg={3}>
                <Paper
                  sx={{
                    p: 2,
                    display: 'flex',
                    flexDirection: 'column',
                    height: 240,
                  }}
                >
                  <Deposits />
                </Paper>
              </Grid> */}
              {/* Recent Orders */}
              {/* <Grid item xs={12}>
                <Paper sx={{ p: 2, display: 'flex', flexDirection: 'column' }}>
                  <Orders />
                </Paper>
              </Grid> */}
            </Grid>
            {/* <Copyright sx={{ pt: 4 }} /> */}
          </Container>
        </Box>
      </Box>
    </ThemeProvider>
  );
}
