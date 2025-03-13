import { Chip, Stack, Typography } from '@mui/material';
import { openUrl } from '@tauri-apps/plugin-opener';
import InfoIcon from '@mui/icons-material/Info';
function WelCome() {
  const databases = [
    {
      name: 'mysql',
      img: 'https://encrypted-tbn0.gstatic.com/images?q=tbn:ANd9GcRK-pSuE46Eo2AAuNQlLg-KAuxpUTkQsCg75A&s',
    },
    {
      name: 'mariadb',
      img: 'https://img1.baidu.com/it/u=2012462183,3210110809&fm=253&fmt=auto&app=138&f=JPEG?w=600&h=411',
    },
    {
      name: 'tidb',
      img: 'https://img1.baidu.com/it/u=2784943579,2058069831&fm=253&fmt=auto&app=138&f=PNG?w=1276&h=500',
    },
    {
      name: 'sqlite',
      img: 'https://img2.baidu.com/it/u=3455858705,150000910&fm=253&fmt=auto&app=138&f=JPEG?w=1055&h=500',
    },
    {
      name: 'oracle',
      img: 'https://img1.baidu.com/it/u=2116707357,1993137839&fm=253&fmt=auto&app=138&f=PNG?w=500&h=272',
    },
    {
      name: 'sql server(microsoft)',
      img: 'https://www.techsoupvietnam.vn/sites/default/files/SQL-Server-2012_0.png',
    },
    {
      name: 'redis',
      img: ' https://banner2.cleanpng.com/20180331/xwe/avh53unx5.webp',
    },
    {
      name: 'mongo',
      img: 'https://mma.prnewswire.com/media/384058/MongoDB_Logo.jpg?p=facebook',
    },
    {
      name: 'postgres',
      img: 'https://pic.rmb.bdstatic.com/bjh/events/a04453e1055b0f0bb0014dd2a247ad62.jpeg@h_1280',
    },
    {
      name: 'cassandra',
      img: 'https://upload.wikimedia.org/wikipedia/commons/thumb/5/5e/Cassandra_logo.svg/800px-Cassandra_logo.svg.png',
    },
    {
      name: 'clickhouse',
      img: 'https://pic.rmb.bdstatic.com/bjh/6cd1c89eb632abaffcde8c19ebca83261118.png',
    },
    {
      name: 'libsql',
      img: 'https://turso.tech/images/blog/the-libsql-glow-up/ugly-old-logo.png',
    },
  ];
  return (
    <Stack spacing={1}>
      <Typography
        variant="h4"
        style={{ fontWeight: '700', textDecoration: 'underline' }}
      >
        dbpal is a database management system that easy to use.
      </Typography>

      <Typography variant="h6" style={{ fontWeight: '700' }}>
        right now we support:
      </Typography>
      <Stack style={{ fontSize: '2rem', fontWeight: '700' }} spacing={1}>
        {databases.map((e) => (
          <Stack
            direction="row"
            spacing={3}
            style={{
              alignItems: 'center',
              padding: '4px',
              textShadow: '8px 8px 2px rgba(113, 113, 113, 0.5)',
            }}
            key={e.name}
          >
            <img
              style={{ backgroundColor: 'white', borderRadius: '8px' }}
              src={e.img}
              width="80px"
              alt={e.name}
            />
            <span>{e.name}</span>
          </Stack>
        ))}
      </Stack>
      <Stack direction="row">
        <Chip
          icon={<InfoIcon></InfoIcon>}
          onClick={() => {
            openUrl('https://github.com/dbpal/docs/blob/main/README.md');
          }}
          label="more information"
          color="info"
        ></Chip>
      </Stack>
    </Stack>
  );
}

export { WelCome };
