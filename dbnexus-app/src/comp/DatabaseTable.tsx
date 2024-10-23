import React, { useState } from 'react';
import { Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination } from '@mui/material';

// 假数据，模拟从数据库加载的数据
const mockData = [
    { id: 1, hash: '0x4ae...', source_chain: 1, seq_num: 0, from_address: '0xa0E...', target_chain: 11 },
    { id: 2, hash: '0x738...', source_chain: 1, seq_num: 2, from_address: '0xa0E...', target_chain: 11 },
    // 更多数据...
];

function DatabaseTable() {
    const [page, setPage] = useState(0);
    const [rowsPerPage, setRowsPerPage] = useState(10);

    const handleChangePage = (event: any, newPage: React.SetStateAction<number>) => {
        setPage(newPage);
    };

    const handleChangeRowsPerPage = (event: { target: { value: string | number; }; }) => {
        setRowsPerPage(+event.target.value);
        setPage(0);
    };

    return (
        <Paper sx={{ width: '100%', m: 2, p: 2, overflow: 'hidden' }}>
            <TableContainer sx={{ maxHeight: 440 }}>
                <Table stickyHeader aria-label="sticky table">
                    <TableHead>
                        <TableRow>
                            <TableCell>ID</TableCell>
                            <TableCell>Hash</TableCell>
                            <TableCell>Source Chain</TableCell>
                            <TableCell>Sequence Number</TableCell>
                            <TableCell>From Address</TableCell>
                            <TableCell>Target Chain</TableCell>
                        </TableRow>
                    </TableHead>
                    <TableBody>
                        {mockData.slice(page * rowsPerPage, page * rowsPerPage + rowsPerPage).map((row) => (
                            <TableRow key={row.id}>
                                <TableCell>{row.id}</TableCell>
                                <TableCell>{row.hash}</TableCell>
                                <TableCell>{row.source_chain}</TableCell>
                                <TableCell>{row.seq_num}</TableCell>
                                <TableCell>{row.from_address}</TableCell>
                                <TableCell>{row.target_chain}</TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </TableContainer>
            <TablePagination
                rowsPerPageOptions={[10, 25, 100]}
                component="div"
                count={mockData.length}
                rowsPerPage={rowsPerPage}
                page={page}
                onPageChange={handleChangePage}
                onRowsPerPageChange={handleChangeRowsPerPage}
            />
        </Paper>
    );
}

export default DatabaseTable;
