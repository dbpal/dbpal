import { Stack } from "@mui/material";
import { DataGrid, GridColDef } from "@mui/x-data-grid";
import { isMacOs } from "./helper";
import { useColumnWidthStore } from "./store";

const shortCuts = getKeyShorts();

function ShowCuts() {
  const shortCutsColumns: GridColDef[] = [{ field: "name" }, { field: "shorts" }];
  const { columnWidths, setColumnWidth } = useColumnWidthStore((s) => s);

  shortCutsColumns.forEach((col) => {
    let width = columnWidths[col.field];

    if (!width) {
      width = 300;
    }
    col.width = width;
  });

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        columns={shortCutsColumns}
        rows={shortCuts}
        getRowId={(e) => {
          return e.name;
        }}
        style={{ height: "100%" }}
        onColumnResize={(resize) => {
          setColumnWidth(resize.colDef.field, resize.width);
        }}
      />
    </Stack>
  );
}

function getKeyShorts(): { name: string; shorts: string }[] {
  if (isMacOs()) {
    return [
      { name: "save file", shorts: "command+s" },
      { name: "open file", shorts: "command+o" },
      { name: "previous tab", shorts: "command+[" },
      { name: "next tab", shorts: "command+]" },
      { name: "close tab", shorts: "command+w" },
      { name: "setting", shorts: "command+," },
      { name: "add connection", shorts: "command+n" },
      { name: "zoom in", shorts: "command+=" },
      { name: "zoom out", shorts: "command+-" },
    ];
  } else {
    return [
      { name: "save file", shorts: "control+s" },
      { name: "open file", shorts: "control+o" },
      { name: "previous tab", shorts: "control+[" },
      { name: "next tab", shorts: "control+]" },
      { name: "close tab", shorts: "control+w" },
      { name: "full screen", shorts: "f11" },
      { name: "setting", shorts: "control+," },
      { name: "add connection", shorts: "control+n" },
      { name: "zoom in", shorts: "control+=" },
      { name: "zoom out", shorts: "control+-" },
    ];
  }
}

export { ShowCuts };
