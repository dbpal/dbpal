import { Box, Stack } from '@mui/material';

import { ResizableColumns } from './ResizableColumns.tsx';
import { useState } from 'react';
import SplitPane, { Pane } from 'react-split-pane';
import './style/react-split-pane.css';

function RedisQuery() {
  const [command, setCommand] = useState('get');
  return (
    <div style={{ display: 'flex' }}>
      <SplitPane split='vertical' minSize={50} defaultSize={1000} step={5}>
        <div>
          {/* <SplitPane split='horizontal' defaultSize='100px'>
          <Pane>You can use a Pane component1111</Pane>
          <div>or you can use a plain old div</div>
        </SplitPane> */}
          11111111111111111111111111111111
        </div>
        <div
          style={{
            height: '100vh',
            position: 'relative',
          }}
        >
          <iframe
            src={`https://www.bilibili.com/`}
            // src='https://www.github.com'
            style={{
              width: '100%',
              height: '100%',
              border: 'none', // 去掉边框
              position: 'absolute',
              top: 0,
              left: 0,
              padding: 0,
              margin: 0,
            }}
            title='Full Size Iframe'
          />
        </div>
        {/* <Pane initialSize='25%' minSize='10%' maxSize='500px'>
          Using a Pane allows you to specify any constraints directly
        </Pane> */}
      </SplitPane>
    </div>
  );
}

export { RedisQuery };
