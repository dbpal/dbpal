import React, { useState } from 'react';
import { Box } from '@mui/material';

const ResizableColumns = ({ leftContent, rightContent, initWidth }) => {
  if (!initWidth) {
    initWidth = 300;
  }
  const [leftWidth, setLeftWidth] = useState(initWidth); // 左列初始宽度
  const [isDragging, setIsDragging] = useState(false); // 拖拽状态

  const handleMouseDown = () => {
    setIsDragging(true);
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  const handleMouseMove = (event) => {
    if (isDragging) {
      const newWidth = event.clientX; // 获取鼠标位置
      if (newWidth > 100 && newWidth < window.innerWidth - 100) {
        setLeftWidth(newWidth); // 设置新的左列宽度
      }
    }
  };

  // 添加事件监听器
  React.useEffect(() => {
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
      <Box
        sx={{
          width: leftWidth,
          padding: 0,
          margin: 0,
        }}
      >
        {leftContent} {/* 显示左侧内容 */}
      </Box>
      <Box
        sx={{
          cursor: 'ew-resize',
          width: '5px',
          backgroundColor: 'gray',
          height: '100%',
          alignSelf: 'stretch',
          '&:hover': {
            backgroundColor: 'darkgray',
          },
        }}
        onMouseDown={handleMouseDown}
      />
      <Box
        sx={{
          flexGrow: 1,
          padding: 1,
        }}
      >
        {rightContent} {/* 显示右侧内容 */}
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
