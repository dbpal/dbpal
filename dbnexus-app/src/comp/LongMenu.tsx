import Menu from '@mui/material/Menu';
import MenuItem from '@mui/material/MenuItem';

export default function LongMenu(props) {
  const { options, handleClose, anchorPosition } = props;

  return (
    <Menu
      id='long-menu'
      anchorReference='anchorPosition'
      anchorPosition={anchorPosition} // Use the saved anchorPosition instead of anchorEl
      anchorOrigin={{
        vertical: 'top',
        horizontal: 'left',
      }}
      transformOrigin={{
        vertical: 'top',
        horizontal: 'left',
      }}
      keepMounted
      open={!!anchorPosition}
      onClose={handleClose}
    >
      {options.map((option) => (
        <MenuItem
          onClick={(event) => {
            option.action(props.index);
            handleClose();
            event.stopPropagation();
          }}
          key={option.label}
          selected={option.label === 'Pyxis'}
        >
          {option.label}
        </MenuItem>
      ))}
    </Menu>
  );
}
