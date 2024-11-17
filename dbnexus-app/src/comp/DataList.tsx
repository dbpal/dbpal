import React, { useState } from 'react';
import {
  Box,
  Typography,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  Collapse,
  TextField,
  Button,
  Select,
  MenuItem,
  InputLabel,
  FormControl,
  IconButton,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import SearchIcon from '@mui/icons-material/Search';
import FolderIcon from '@mui/icons-material/Folder';
import ExpandLess from '@mui/icons-material/ExpandLess';
import ExpandMore from '@mui/icons-material/ExpandMore';

function DataList({ addEntity }: { addEntity: (name: string, fields: any[]) => void }) {
  const [database, setDatabase] = useState('');
  const [filter, setFilter] = useState('');
  const [openEntity, setOpenEntity] = useState<string | null>(null);

  const handleDatabaseChange = (event: { target: { value: string } }) => {
    setDatabase(event.target.value);
  };

  const handleFilterChange = (event: { target: { value: string } }) => {
    setFilter(event.target.value);
  };

  const handleEntityClick = (entityName: string, fields: any[]) => {
    addEntity(entityName, fields); // 调用父组件方法，传递实体名称和字段
    setOpenEntity(openEntity === entityName ? null : entityName);
  };

  // 示例数据
  const entities = [
    {
      name: 'aBTCDepositEvent',
      fields: [
        { name: 'id', type: 'int(10) unsigned' },
        { name: 'hash', type: 'varchar(100)' },
        // 更多字段...
      ],
    },
    {
      name: 'aBTCEvent',
      fields: [
        { name: 'event_id', type: 'int(10) unsigned' },
        { name: 'event_type', type: 'varchar(50)' },
      ],
    },
    // 更多实体...
  ];

  return (
    <Box
      sx={{
        width: 235,
        bgcolor: 'background.paper',
        color: 'text.primary',
        p: 2,
      }}
    >
      <FormControl fullWidth sx={{ mb: 2 }}>
        <InputLabel>Select a database...</InputLabel>
        <Select value={database} onChange={handleDatabaseChange}>
          <MenuItem value="Database1">Database1</MenuItem>
          <MenuItem value="Database2">Database2</MenuItem>
        </Select>
      </FormControl>
      <TextField
        fullWidth
        label="Filter"
        variant="outlined"
        size="small"
        value={filter}
        onChange={handleFilterChange}
        sx={{ mb: 2 }}
      />
      <Typography variant="subtitle2" sx={{ mb: 1 }}>
        ENTITIES ({entities.length})
      </Typography>
      <List dense>
        {entities.map((entity) => (
          <React.Fragment key={entity.name}>
            <ListItem
              button
              onClick={() => handleEntityClick(entity.name, entity.fields)}
            >
              <ListItemText primary={entity.name} />
              {openEntity === entity.name ? <ExpandLess /> : <ExpandMore />}
            </ListItem>
          </React.Fragment>
        ))}
      </List>
    </Box>
  );
}

export default DataList;
