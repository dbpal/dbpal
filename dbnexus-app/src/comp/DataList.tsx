import React, { useState } from 'react';
import { Box, Typography, List, ListItem, TextField, Button, Select, MenuItem, InputLabel, FormControl } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import SearchIcon from '@mui/icons-material/Search';

function DataList() {
    const [database, setDatabase] = useState('');
    const [filter, setFilter] = useState('');

    const handleDatabaseChange = (event: { target: { value: React.SetStateAction<string>; }; }) => {
        setDatabase(event.target.value);
    };

    const handleFilterChange = (event: { target: { value: React.SetStateAction<string>; }; }) => {
        setFilter(event.target.value);
    };

    return (
        <Box sx={{ width: 240, bgcolor: 'background.paper', color: 'text.primary', p: 2 }}>
            <Box sx={{ mb: 2 }}>
                <FormControl fullWidth>
                    <InputLabel id="database-select-label">Select a database...</InputLabel>
                    <Select
                        labelId="database-select-label"
                        id="database-select"
                        value={database}
                        label="Select a database..."
                        onChange={handleDatabaseChange}
                    >
                        <MenuItem value="Database1">Database1</MenuItem>
                        <MenuItem value="Database2">Database2</MenuItem>
                        {/* More databases */}
                    </Select>
                </FormControl>
            </Box>
            <TextField
                fullWidth
                id="search"
                label="Filter"
                variant="outlined"
                size="small"
                value={filter}
                onChange={handleFilterChange}
                InputProps={{

                }}
            />
            <Typography variant="subtitle2" sx={{ mt: 2, mb: 1 }}>
                ENTITIES (0)
            </Typography>

        </Box>
    );
}

export default DataList;
