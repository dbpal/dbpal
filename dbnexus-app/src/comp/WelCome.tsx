import {
  Divider,
  Link,
  List,
  ListItem,
  Stack,
  Typography,
} from '@mui/material';
import { openUrl } from '@tauri-apps/plugin-opener';

function WelCome() {
  const databases = [
    'mysql',
    'mariadb',
    'tidb',
    'sqlite',
    'oracle',
    'sql server(microsoft)',
    'redis',
    'mongo',
    'postgres',
  ];
  return (
    <Stack spacing={1}>
      <Typography variant="h5">
        dbpal is a database management system that easy to use.
      </Typography>
      <Divider></Divider>
      <Typography variant="h6">rightnow we support:</Typography>
      <List>
        {databases.map((e) => (
          <ListItem key={e}>{e}</ListItem>
        ))}
      </List>

      <Link
        style={{ cursor: 'pointer' }}
        href="#"
        onClick={() => {
          openUrl('https://github.com/dbpal/docs/blob/main/README.md');
        }}
      >
        more information
      </Link>
    </Stack>
  );
}

export { WelCome };
