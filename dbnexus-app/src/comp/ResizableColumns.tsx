import React, { useEffect, useRef, useState } from 'react';
import { Box } from '@mui/material';
import ReactDOM from 'react-dom/client';

const ResizableColumns = ({ leftContent, rightContent, initWidth }) => {
  if (!initWidth) {
    initWidth = 300;
  }
  const [leftWidth, setLeftWidth] = useState(initWidth); // 左列初始宽度
  const [isDragging, setIsDragging] = useState(false); // 拖拽状态
  const iframeRef = useRef(null);

  const handleMouseDown = () => {
    setIsDragging(true);
    disableIframe();
  };

  const handleMouseUp = () => {
    setIsDragging(false);
    enableIframe();
  };

  const handleMouseMove = (event) => {
    if (isDragging) {
      const newWidth = event.clientX; // 获取鼠标位置
      if (newWidth > 100 && newWidth < window.innerWidth - 100) {
        setLeftWidth(newWidth); // 设置新的左列宽度
      }
    }
  };

  const disableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'none'; // 禁用 iframe 交互
    }
  };

  const enableIframe = () => {
    if (iframeRef.current) {
      iframeRef.current.style.pointerEvents = 'auto'; // 启用 iframe 交互
    }
  };

  useEffect(() => {
    if (isDragging) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
    } else {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    }
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging]);

  return (
    <Box sx={{ display: 'flex', height: '100vh' }}>
      {/* 左侧内容 */}
      <Box
        sx={{
          width: leftWidth,
          padding: '10px',
          backgroundColor: '#f0f0f0',
        }}
      >
        {leftContent}
      </Box>

      {/* 分隔条 */}
      <Box
        sx={{
          cursor: 'ew-resize',
          width: '5px',
          backgroundColor: 'gray',
          height: '100%',
          '&:hover': {
            backgroundColor: 'darkgray',
          },
        }}
        onMouseDown={handleMouseDown}
      />

      {/* 右侧内容 */}
      <Box
        sx={{
          flexGrow: 1,
          padding: '10px',
          backgroundColor: '#e0e0e0',
        }}
      >
        {rightContent}
        <iframe
          ref={iframeRef}
          src="https://redis.io/docs/latest/commands/get/"
          style={{
            width: '100%',
            height: '100%',
            border: 'none',
          }}
          title="iframe"
        />
      </Box>
    </Box>
  );
};

// 使用示例
const TestApp = () => {
  return (
    <ResizableColumns
      leftContent={<div>左侧内容</div>}
      rightContent={<div>右侧内容</div>}
      initWidth={200}
    />
  );
};

export { ResizableColumns, TestApp };

// const root = ReactDOM.createRoot(document.getElementById('root'));
// root.render(<ResizableColumns
//   leftContent={<div>左侧内容</div>}
//   rightContent={<div>右侧内容</div>}
//   initWidth={100}
// />
// )