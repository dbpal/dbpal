import React from 'react';
import Box from '@mui/material/Box';
import Paper from '@mui/material/Paper';
import { DataGrid, GridColDef, GridRowsProp } from '@mui/x-data-grid';
import { useTheme } from '@mui/material/styles';

const columns: GridColDef[] = [
  { field: 'id', headerName: 'ID', width: 90, headerClassName: 'header-style' },
  { field: 'hash', headerName: 'Hash', width: 150, editable: true, headerClassName: 'header-style' },
  { field: 'source_chain', headerName: 'Source Chain', type: 'number', width: 150, editable: true, headerClassName: 'header-style' },
  { field: 'seq_num', headerName: 'Sequence Number', type: 'number', width: 150, editable: true, headerClassName: 'header-style' },
  { field: 'from_address', headerName: 'From Address', width: 200, editable: true, headerClassName: 'header-style' },
  { field: 'target_chain', headerName: 'Target Chain', type: 'number', width: 150, editable: true, headerClassName: 'header-style' },
];

const rows: GridRowsProp = [
  { id: 1, hash: '0x4ae...', source_chain: 1, seq_num: 0, from_address: '0xa0E...', target_chain: 11 },
  { id: 2, hash: '0x738...', source_chain: 1, seq_num: 2, from_address: '0xa0E...', target_chain: 11 },
  { id: 3, hash: '0xfee...', source_chain: 1, seq_num: 100000000001, from_address: '0x409...', target_chain: 11 },
  { id: 4, hash: '0x350...', source_chain: 1, seq_num: 1, from_address: '0xa0E...', target_chain: 11 },
  { id: 5, hash: '0xf391...', source_chain: 1, seq_num: 3, from_address: '0xa0E...', target_chain: 11 },
];

export default function DatabaseTable() {
  const theme = useTheme();

  return (
    <Box
      sx={{
        flexGrow: 1,
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        padding: 2,
        mt: 1,
        ml: 4,
        mb: 0,
      }}
    >
      <Paper
        sx={{
          height: '83vh',
          width: '83vw',
          padding: 2,
          backgroundColor: theme.palette.mode === 'dark' ? '#1e1e1e' : 'white',
          color: theme.palette.mode === 'dark' ? 'white' : 'black',
        }}
        elevation={3}
      >
        <DataGrid
          rows={rows}
          columns={columns}
          initialState={{
            pagination: {
              paginationModel: {
                pageSize: 5,
              },
            },
          }}
          pageSizeOptions={[5, 10, 20]}
          checkboxSelection
          disableRowSelectionOnClick
          sx={{
            '.MuiDataGrid-root': {
              backgroundColor: theme.palette.mode === 'dark' ? '#1e1e1e' : 'white',
              color: theme.palette.mode === 'dark' ? 'white' : 'black',
            },
            '.MuiDataGrid-cell': {
              borderBottom: theme.palette.mode === 'dark' ? '1px solid #333' : '1px solid #e0e0e0',
            },
            '.MuiDataGrid-columnHeaders': {
              backgroundColor: theme.palette.mode === 'dark' ? '#333' : '#f5f5f5',
              color: theme.palette.mode === 'dark' ? 'white' : 'black',
              borderBottom: theme.palette.mode === 'dark' ? '1px solid #333' : '1px solid #e0e0e0',
            },
            '.MuiDataGrid-footerContainer': {
              backgroundColor: theme.palette.mode === 'dark' ? '#333' : '#f5f5f5',
              color: theme.palette.mode === 'dark' ? 'white' : 'black',
            },
            '.MuiDataGrid-checkboxInput': {
              color: theme.palette.mode === 'dark' ? 'white' : 'black',
            },
          }}
        />
      </Paper>
    </Box>
  );
}
