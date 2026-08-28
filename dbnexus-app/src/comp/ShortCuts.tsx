import { Stack } from "@mui/material";
import { useMemo } from "react";
import { isMacOs } from "./helper";
import { useColumnWidthStore } from "./store";
import { DataGrid, GridColDef } from "./datagridExport";
import { AppDataGridPagination } from "./CommonComp2";

const shortCuts = getKeyShorts();

function ShowCuts() {
  const { getColumnWidth } = useColumnWidthStore();
  const shortCutsColumns = useMemo((): GridColDef[] => {
    return ["name", "shorts"].map((field) => ({
      field,
      width: getColumnWidth(field) || 300,
    }));
  }, [getColumnWidth]);
  const slots = useMemo(() => ({ pagination: AppDataGridPagination }), []);
  const style = useMemo(() => ({ height: "100%" }), []);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        columns={shortCutsColumns}
        rows={shortCuts}
        slots={slots}
        style={style}
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
  baseShorts.forEach((e, index) => {
    e["id"] = index;
  });

  return baseShorts;
}

export { ShowCuts };
