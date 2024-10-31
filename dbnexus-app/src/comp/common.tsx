import {
  Box,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Select,
  Stack,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

const errOkHoldTime = 5000;

// TOOD 适配黑暗模式
const modalStyle = {
  position: 'absolute',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  bgcolor: 'background.paper',
  border: '1px solid #000',
  boxShadow: 24,
  p: 4,
};

function SelectPageSize({
  width = '130px',
  variant = 'outlined',
  tital = 'select page size',
  pageSize = 50,
  setPageSize = (_: number) => {
    // a function which will do nothing.
  },
}) {
  return (
    <Box width={width}>
      <FormControl fullWidth variant={variant}>
        <InputLabel id='simple-select-label'>{tital}</InputLabel>
        <Select
          labelId='simple-select-label'
          id='simple-select'
          value={pageSize}
          onChange={(e) => {
            setPageSize(e.target.value as number);
          }}
          label='select page size'
        >
          <MenuItem value={50}>50</MenuItem>
          <MenuItem value={100}>100</MenuItem>
          <MenuItem value={200}>200</MenuItem>
          <MenuItem value={500}>500</MenuItem>
          <MenuItem value={1000}>1000</MenuItem>
          <MenuItem value={2000}>2000</MenuItem>
          <MenuItem value={5000}>5000</MenuItem>
          <MenuItem value={10000}>10000</MenuItem>
        </Select>
      </FormControl>
    </Box>
  );
}

function CloseButton({ handleClose = null }) {
  return (
    <IconButton onClick={handleClose} edge='end' aria-label='delete'>
      <CloseIcon />
    </IconButton>
  );
}

function CloseSpace({ handleClose = null }) {
  return (
    <Stack direction='row' alignItems='right' justifyContent='space-between'>
      <span></span>
      <CloseButton handleClose={handleClose} />
    </Stack>
  );
}

export { errOkHoldTime, modalStyle, SelectPageSize, CloseButton, CloseSpace };
