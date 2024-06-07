import React from 'react';
import { TextField, Button, Typography, Box, Paper } from '@mui/material';

function QueryForm() {
    return (
        <Paper sx={{ display: 'flex', height: '100%', m: 2, p: 2 }}>
            <Box sx={{ width: '25%', borderRight: '1px solid #ccc', p: 2 }}>
                {/* Database and entities display logic */}
            </Box>
            <Box sx={{ flexGrow: 1, p: 2 }}>
                <Paper variant="outlined" sx={{ height: '100%', overflow: 'auto' }}>
                    <Typography variant="h6" sx={{ p: 2 }}>Query #1</Typography>
                    <TextField
                        fullWidth
                        multiline
                        rows={16}
                        placeholder="Type SQL query here..."
                        variant="outlined"
                    />
                    <Button variant="contained" sx={{ m: 2 }}>
                        Run
                    </Button>
                </Paper>
            </Box>
        </Paper>
    );
}

export default QueryForm;
