import { Box, Stack } from '@mui/material';

import { ResizableColumns } from './ResizableColumns.tsx';
import { useState } from 'react';

function RedisQuery() {
  const [command, setCommand] = useState('get');
  return (
    <ResizableColumns
      initWidth={500}
      leftContent={
        <>
          <div
            style={{
              width: '100%',
              height: '60vh',
              overflow: 'auto',
              resize: 'vertical',
              border: '1px solid black',
            }}
          >
            111
          </div>
          <div
            style={{
              width: '100%',
              height: '40vh',
            }}
          >
            query resp
          </div>
        </>
      }
      rightContent={
        // right
        <Box
          sx={{
            width: '100%',
            height: '100vh', // 设置为视口高度，或根据需要调整
            position: 'relative',
          }}
        >
          <iframe
            src={`https://redis.io/docs/latest/commands/${command}/`}
            style={{
              width: '100%',
              height: '100%',
              border: 'none', // 去掉边框
              position: 'absolute',
              top: 0,
              left: 0,
            }}
            title='Full Size Iframe'
          />
        </Box>
      }
    ></ResizableColumns>
  );
}

export { RedisQuery };
