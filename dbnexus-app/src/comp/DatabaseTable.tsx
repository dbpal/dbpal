import React, { useState } from 'react';
import { Box, Paper, Tab, Tabs, IconButton, Typography } from '@mui/material';
import { DataGrid, GridColDef, GridRowsProp } from '@mui/x-data-grid';
import CloseIcon from '@mui/icons-material/Close';
import { useTheme } from '@mui/material/styles';

// 封装 DataGrid 的创建逻辑
const createDataGrid = (rows: GridRowsProp, columns: GridColDef[], gridName: string) => (
  <Box sx={{ height: '100%' }}>
    <Typography variant="h6" sx={{ mb: 2 }}>
      {gridName}
    </Typography>
    <DataGrid
      rows={rows}
      columns={columns}
      pageSizeOptions={[5, 10, 20]}
      checkboxSelection
      disableRowSelectionOnClick
      sx={{
        height: 'calc(100% - 32px)',
        backgroundColor: '#fff',
        color: '#000',
        '.MuiDataGrid-columnHeaders': {
          backgroundColor: '#f5f5f5',
        },
      }}
    />
  </Box>
);

export default function DatabaseTable({ addEntity }: { addEntity: (name: string) => void }) {
  const theme = useTheme();

  // 存储 DataGrid 的名称及内容
  const [dataGrids, setDataGrids] = useState<{ name: string; content: JSX.Element }[]>([]);
  const [activeTab, setActiveTab] = useState(0);

  // 切换 Tab
  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setActiveTab(newValue);
  };

  // 添加新的 DataGrid
  const addDataGrid = (name: string, rows: GridRowsProp, columns: GridColDef[]) => {
    const newGrid = {
      name,
      content: createDataGrid(rows, columns, name),
    };
    setDataGrids((prev) => [...prev, newGrid]);
    setActiveTab(dataGrids.length); // 自动切换到新添加的 Tab
  };

  // 移除 DataGrid
  const removeDataGrid = (index: number) => {
    setDataGrids((prev) => prev.filter((_, i) => i !== index));
    setActiveTab((prev) => (prev >= index ? Math.max(prev - 1, 0) : prev));
  };

  return (
    <Box
      sx={{
        flexGrow: 1,
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'flex-start',
        padding: 2,
        ml: 4,
      }}
    >
      <Paper
        sx={{
          height: '85vh',
          width: '80vw',
          padding: 2,
          backgroundColor: theme.palette.mode === 'dark' ? '#1e1e1e' : 'white',
        }}
        elevation={3}
      >
        {/* Tab 切换 */}
        <Tabs
          value={activeTab}
          onChange={handleTabChange}
          variant="scrollable"
          scrollButtons="auto"
          sx={{
            borderBottom: '1px solid #ddd',
            mb: 2,
            '.MuiTab-root': { textTransform: 'none' },
          }}
        >
          {dataGrids.map((grid, index) => (
            <Tab
              key={index}
              label={
                <Box sx={{ display: 'flex', alignItems: 'center' }}>
                  {grid.name}
                  <IconButton
                    size="small"
                    onClick={(e) => {
                      e.stopPropagation(); // 防止点击关闭按钮切换 Tab
                      removeDataGrid(index);
                    }}
                    sx={{ ml: 1 }}
                  >
                    <CloseIcon fontSize="small" />
                  </IconButton>
                </Box>
              }
            />
          ))}
        </Tabs>

        {/* 显示当前激活的 DataGrid */}
        <Box sx={{ height: '75vh', mt: 2 }}>
          {dataGrids[activeTab]?.content || <Typography>No DataGrid Available</Typography>}
        </Box>
      </Paper>
    </Box>
  );
}
