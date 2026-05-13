import { Stack } from "@mui/material";
import { isMacOs } from "./helper";
import { useColumnWidthStore } from "./store";
import { DataGrid, GridColDef } from "./datagridExport";
import { AppDataGridPagination, AppDataGridToolBar } from "./CommonComp2";

const shortCuts = getKeyShorts();

function ShowCuts() {
  const shortCutsColumns: GridColDef[] = [{ field: "name" }, { field: "shorts" }];
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();

  shortCutsColumns.forEach((col) => {
    let width = getColumnWidth(col.field);

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
        slots={{ pagination: AppDataGridPagination, toolbar: AppDataGridToolBar }}
        style={{ height: "100%" }}
        onColumnResize={(resize) => {
          setColumnWidth(resize.colDef.field, resize.width);
        }}
      />
    </Stack>
  );
}

function getKeyShorts(): { name: string; shorts: string }[] {
  const isMac = isMacOs();
  // 定义修饰键符号
  const CMD = isMac ? "⌘" : "Ctrl";

  // const _ALT = isMac ? "⌥" : "Alt";
  // const _SHIFT = isMac ? "⇧" : "Shift";

  const baseShorts = [
    { name: "save file", shorts: `${CMD}+S` },
    { name: "open file", shorts: `${CMD}+O` },
    { name: "previous tab", shorts: `${CMD}+[` },
    { name: "next tab", shorts: `${CMD}+]` },
    { name: "close tab", shorts: `${CMD}+W` },
    { name: "setting", shorts: `${CMD}+,` },
    { name: "add connection", shorts: `${CMD}+N` },
    { name: "zoom in", shorts: `${CMD}++` }, // 使用 ++ 表示加号键
    { name: "zoom out", shorts: `${CMD}+-` },
    { name: "editor markdown", shorts: `${CMD}+B` },
    { name: "prev page", shorts: `${CMD}+ArrowUp` },
    { name: "next page", shorts: `${CMD}+ArrowDown` },
  ];

  // 针对非 Mac 平台的特殊补充
  if (!isMac) {
    baseShorts.push({ name: "full screen", shorts: "F11" });
  }

  return baseShorts;
}

export { ShowCuts };
