import { FormControl, InputLabel, Select, MenuItem } from '@mui/material';

const LogLevelSel = ({ handleChange, level }) => {
  let levels = ['trace', 'debug', 'info', 'warn', 'error'];

  return (
    <FormControl style={{ width: '100px' }}>
      <InputLabel id="dropdown-label">select a level</InputLabel>
      <Select
        labelId="dropdown-label"
        value={level}
        onChange={(e) => {
          handleChange(e.target.value);
        }}
        variant="outlined"
        size="small"
      >
        {levels.map((e) => (
          <MenuItem value={e} key={e}>
            {e}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};

export { LogLevelSel };
