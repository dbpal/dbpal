import {
  Checkbox,
  Chip,
  Divider,
  FormControlLabel,
  IconButton,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { filesize } from "filesize";
import { useEffect, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import { Channel } from "@tauri-apps/api/core";
import { OpenFileWithDefaultApp } from "./CommonComp";
import {
  DataSourceKind,
  generateDefaultFilename,
  getSupportExportKind,
} from "./helper";

import { cancelTask } from "./handlers";
import { CancelButton, StackRowCenter } from "./CommonComp2";
import { useExportConfigStore, useNotification } from "./store";
import { commands, ExportKind, ShowTableParameter } from "./bindings";
import { uuidv4 } from "./Common";
function IncludeHeader({ includeHeader, setIncludeHeader }) {
  return (
    <FormControlLabel
      control={
        <Checkbox
          checked={includeHeader}
          onChange={(e) => {
            setIncludeHeader(e.target.checked);
          }}
          name="myCheckbox"
          color="primary"
        />
      }
      label="Include Header(if applicable)"
    />
  );
}

function ExportView(props: {
  name: string;
  exportOP: (
    kind: ExportKind,
    file: string,
    taskId: string,
    c: Channel<number>,
  ) => Promise<void>;
  supportKind: string[];
}) {
  const { exportOP, supportKind, name } = props;

  const {
    includeHeader,
    delimiter,
    setDelimiter,
    setIncludeHeader,
    jsonPretty,
    setJsonPretty,
    includeTableSchema,
    setIncludeTableSchema,
    exportKind,
    setExportKind,
  } = useExportConfigStore();

  const { showNotification } = useNotification();
  const [loading, setLoading] = useState(false);
  const [file, setFile] = useState<string | null>(null);
  const taskId = useRef(uuidv4());
  const exportedSize = useRef(null);
  const makeConfig = (): ExportKind | null => {
    switch (exportKind) {
      case "csv":
        if (delimiter.length > 1) {
          showNotification({
            severity: "error",
            title: "delimiter expect one character",
          });
          return;
        }
        return {
          export_kind: "csv",
          include_header: includeHeader,
          delimiter: delimiter,
        };
      case "tsv":
        return { export_kind: "csv", include_header: includeHeader, delimiter: "\t" };
      case "sql":
        return { export_kind: "sql", include_table_schema: includeTableSchema };
      case "json":
        return { export_kind: "json", pretty: jsonPretty };
      case "jsonline":
        return { export_kind: "jsonline" };
      case "xlsx":
        return { export_kind: "xlsx", include_header: includeHeader };
      case "html":
        return { export_kind: "html", include_header: includeHeader };
      case "markdown":
        return { export_kind: "markdown" };
    }
  };

  useEffect(() => {
    if (!supportKind.find((e) => e == exportKind)) {
      setExportKind(supportKind[0]);
    }
  }, [exportKind, setExportKind, supportKind]);
  const handleExport = async () => {
    const config = makeConfig();
    if (!config) {
      return;
    }
    const file = await save({
      defaultPath: generateDefaultFilename(name, exportKind),
    });
    if (!file) {
      return;
    }
    exportedSize.current.innerText = "";
    const c = new Channel<number>((x) => {
      exportedSize.current.innerText = `exported ${filesize(x)}`;
    });
    try {
      setLoading(true);
      await exportOP(config, file, taskId.current, c);
    } catch (e) {
      showNotification({ severity: "warning", title: "export failure", message: e });
      return;
    } finally {
      setLoading(false);
    }
    showNotification({ severity: "success", title: "export OK" });
    setFile(file);
  };
  useEffect(() => {
    setFile("");
  }, [exportKind]);
  return (
    <>
      <Stack direction="row">
        {supportKind.map((e) => (
          <Chip
            onClick={() => {
              setExportKind(e);
            }}
            key={e}
            label={e}
            color={e == exportKind ? "success" : "default"}
          />
        ))}
      </Stack>
      {["csv", "tsv"].includes(exportKind) && (
        <Stack>
          <IncludeHeader
            includeHeader={includeHeader}
            setIncludeHeader={setIncludeHeader}
          />
          {exportKind == "csv" && (
            <Stack direction="row">
              <TextField
                value={delimiter}
                onChange={(e) => {
                  setDelimiter(e.target.value);
                }}
                label="delimiter"
                error={delimiter.length > 1}
                helperText="only accept one character"
              />
            </Stack>
          )}
        </Stack>
      )}

      {exportKind == "xlsx" && (
        <Stack>
          <IncludeHeader
            includeHeader={includeHeader}
            setIncludeHeader={setIncludeHeader}
          />
        </Stack>
      )}
      {exportKind == "sql" && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={includeTableSchema}
                onChange={(e) => {
                  setIncludeTableSchema(e.target.checked);
                }}
                name="myCheckbox"
                color="primary"
              />
            }
            label="Include Table Schema(if applicable)"
          />
        </Stack>
      )}
      {exportKind == "json" && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={jsonPretty}
                onChange={(e) => {
                  setJsonPretty(e.target.checked);
                }}
                name="pretty"
                color="primary"
              />
            }
            label="pretty"
          />
        </Stack>
      )}
      {/* json line have no settings */}

      {exportKind == "jsonline" && <></>}

      {exportKind == "html" && (
        <IncludeHeader
          includeHeader={includeHeader}
          setIncludeHeader={setIncludeHeader}
        />
      )}

      <Divider />

      <StackRowCenter>
        <IconButton
          onClick={async () => {
            handleExport();
          }}
          color="primary"
          loading={loading}>
          <ArrowCircleDownIcon />
        </IconButton>
        {loading && (
          <CancelButton
            handleCancel={() => {
              cancelTask(taskId.current);
              exportedSize.current.innerText = "";
            }}
          />
        )}
        <Typography ref={exportedSize} />
        <OpenFileWithDefaultApp file={file} />
      </StackRowCenter>
    </>
  );
}

function ExportTableOrView(props: {
  uuid: string;
  name: string;
  isTable: boolean;
  para?: ShowTableParameter;
  kind: DataSourceKind;
}) {
  const { uuid, name, kind, isTable, para } = props;

  return (
    <ExportView
      exportOP={async (kind, filepath, taskId, sizeEmitter) => {
        return await commands.exportTableOrView(
          uuid,
          taskId,
          sizeEmitter,
          name,
          para,
          filepath,
          kind,
        );
      }}
      supportKind={getSupportExportKind(kind, isTable)}
      name={name}
    />
  );
}

export { ExportView, ExportTableOrView };
