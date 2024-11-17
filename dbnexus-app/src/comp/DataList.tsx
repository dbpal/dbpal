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

  const handleDatabaseChange = (event: {
    target: { value: React.SetStateAction<string> };
  }) => {
    setDatabase(event.target.value);
  };

  const handleFilterChange = (event: {
    target: { value: React.SetStateAction<string> };
  }) => {
    setFilter(event.target.value);
  };

  const handleEntityClick = (entityName: string, fields?: { name: string; type: string; }[]) => {
    setOpenEntity(openEntity === entityName ? null : entityName);
  };

  // 示例数据
  const entities = [
    {
      name: 'aBTCDepositEvent',
      fields: [
        { name: 'id', type: 'int(10) unsigned' },
        { name: 'hash', type: 'varchar(100)' },
        { name: 'sequence_number', type: 'varchar(66)' },
        { name: 'address', type: 'varchar(66)' },
        { name: 'amount', type: 'bigint(20) unsigned' },
        { name: 'version', type: 'bigint(20) unsigned' },
        { name: 'timestamp', type: 'bigint(20) unsigned' },
        { name: 'state', type: 'int(10) unsigned' },
        { name: 'created', type: 'timestamp' },
        { name: 'updated', type: 'timestamp' },
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
      <Box sx={{ mb: 2 }}>
        <FormControl fullWidth>
          <InputLabel id='database-select-label'>Select a database...</InputLabel>
          <Select
            labelId='database-select-label'
            id='database-select'
            value={database}
            label='Select a database...'
            onChange={handleDatabaseChange}
          >
            <MenuItem value='Database1'>Database1</MenuItem>
            <MenuItem value='Database2'>Database2</MenuItem>
            {/* More databases */}
          </Select>
        </FormControl>
      </Box>
      <TextField
        fullWidth
        id='search'
        label='Filter'
        variant='outlined'
        size='small'
        value={filter}
        onChange={handleFilterChange}
      />
      <Typography variant='subtitle2' sx={{ mt: 2, mb: 1 }}>
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

      {/* <List component="nav" dense>
        {entities.map((entity) => (
          <React.Fragment key={entity.name}>
            <ListItem onClick={() => handleEntityClick(entity.name)}>
              <ListItemText primary={entity.name} />
              {openEntity === entity.name ? <ExpandLess /> : <ExpandMore />}
            </ListItem>
            <Collapse in={openEntity === entity.name} timeout="auto" unmountOnExit>
              <List component="div" disablePadding>
                {entity.fields.map((field) => (
                  <ListItem sx={{ m: 0, p: 0, pl: 4 }} key={field.name} >
                    <ListItemText
                      primary={field.name}
                      secondary={field.type}
                      primaryTypographyProps={{ fontSize: '0.9em' }}
                      secondaryTypographyProps={{ fontSize: '0.75em', color: 'text.secondary' }}
                    />
                  </ListItem>
                ))}
              </List>
            </Collapse>
          </React.Fragment>
        ))}
      </List> */}

      <Button
        variant="outlined"
        startIcon={<AddIcon />}
        sx={{ mt: 2 }}
      >
        Add Entity
      </Button>
    </Box>
  );
}

export default DataList;
