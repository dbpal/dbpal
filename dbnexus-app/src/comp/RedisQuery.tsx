import { Box, Stack } from '@mui/material';
import { useState, useRef, useEffect } from 'react';
import SplitPane from 'react-split-pane';
import './style/react-split-pane.css';

function RedisQuery() {
  const [command, setCommand] = useState('get');
  const [isDragging, setIsDragging] = useState(false);
  const iframeRef = useRef(null);

  // 禁用 iframe 的鼠标事件
  const disableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'none';
    }
  };

  // 启用 iframe 的鼠标事件
  const enableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'auto';
    }
  };

  useEffect(() => {
    if (isDragging) {
      disableIframe();
    } else {
      enableIframe();
    }
  }, [isDragging]);

  return (
    <div style={{ display: 'flex' }}>
      <SplitPane
        split="vertical"
        minSize={50}
        defaultSize={1000}
        step={5}
        onDragStarted={() => setIsDragging(true)} // 拖动开始
        onDragFinished={() => setIsDragging(false)} // 拖动结束
      >
        <div>
          11111111111111111111111111111111
        </div>
        <div
          style={{
            height: '100vh',
            position: 'relative',
          }}
        >
          <iframe
            ref={iframeRef} // 绑定 iframe 引用
            src={`https://www.bilibili.com/`}
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
            title="Full Size Iframe"
          />
        </div>
      </SplitPane>
    </div>
  );
}

export { RedisQuery };
