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
import { useContext, useEffect, useRef, useState } from "react";
import types from "./ApiStruct";
import { NotificationContext } from "./Notification";
import { save } from "@tauri-apps/plugin-dialog";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import { Channel, invoke } from "@tauri-apps/api/core";
import { ModalHeader, OpenFileWithDefaultApp } from "./CommonComp";
import {
  generateDefaultFilename,
  generateRandomString,
  SettingContext,
} from "./helper";

import { cancelTask } from "./handlers";
import { CancelButton, StackRowCenter } from "./CommonComp2";
function IncludeHeader({ includeHeader, setIncludeHeader }) {
  return (
    <FormControlLabel
      control={
        <Checkbox
          checked={includeHeader}
          onChange={e => {
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

let lastExportKind = "csv";
function ExportView(props: {
  name: string;
  exportOP: (
    kind: types.ExportKind,
    file: string,
    taskId: string,
    c: Channel<number>,
  ) => Promise<void>;
  supportKind: string[];
  handleClose: () => void;
}) {
  const { exportOP, supportKind, name, handleClose } = props;
  const lastKind = supportKind.find(k => k === lastExportKind);
  const [kind, setKind] = useState(lastExportKind ? lastKind : supportKind[0]);
  const setting = useContext(SettingContext);
  const [includeHeader, setIncludeHeader] = useState(setting.export.includeHeader);
  const [delimiter, setDelimiter] = useState(setting.export.csvDelimiter);
  const [pretty, setPretty] = useState(true);
  const [includeTableSchema, setIncludeTableSchema] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [file, setFile] = useState<string | null>(null);
  const taskId = useRef(generateRandomString(5));
  useEffect(() => {
    lastExportKind = kind;
  }, [kind]);
  const exportedSize = useRef(null);
  const makeConfig = (): types.ExportKind | null => {
    switch (kind) {
      case "csv":
        if (delimiter.length > 1) {
          showNotification("error", "delimiter expect one character");
          return;
        }
        return {
          export_kind: "csv",
          include_header: includeHeader,
          delimiter,
        };
      case "sql":
        return { export_kind: "sql", include_table_schema: includeTableSchema };
      case "json":
        return { export_kind: "json", pretty };
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

  const handleExport = async () => {
    const config = makeConfig();
    if (!config) {
      return;
    }
    const file = await save({
      defaultPath: generateDefaultFilename(name, kind),
    });
    if (!file) {
      return;
    }
    exportedSize.current.innerText = "";
    const c = new Channel<number>(x => {
      exportedSize.current.innerText = `exported ${filesize(x)}`;
    });
    try {
      setLoading(true);
      await exportOP(config, file, taskId.current, c);
    } catch (e) {
      showNotification("warning", "export failure", e);
      return;
    } finally {
      setLoading(false);
    }
    showNotification("success", "export OK");
    setFile(file);
  };
  useEffect(() => {
    setFile("");
  }, [kind]);
  return (
    <Stack>
      <ModalHeader
        title={`export ${name}`}
        handleClose={() => {
          if (loading) {
            showNotification("warning", "exporting, please wait");
            return;
          }
          handleClose();
        }}
      />
      <Stack direction="row">
        {supportKind.map(e => (
          <Chip
            onClick={() => {
              setKind(e);
            }}
            key={e}
            label={e}
            color={e == kind ? "success" : "default"}
          />
        ))}
      </Stack>
      {kind == "csv" && (
        <Stack>
          <IncludeHeader
            includeHeader={includeHeader}
            setIncludeHeader={setIncludeHeader}
          />
          <Stack direction="row">
            <TextField
              value={delimiter}
              onChange={e => {
                setDelimiter(e.target.value);
              }}
              label="delimiter"
              error={delimiter.length > 1}
              helperText="only accept one character"
            />
          </Stack>
        </Stack>
      )}
      {kind == "xlsx" && (
        <Stack>
          <IncludeHeader
            includeHeader={includeHeader}
            setIncludeHeader={setIncludeHeader}
          />
        </Stack>
      )}
      {kind == "sql" && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={includeTableSchema}
                onChange={e => {
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
      {kind == "json" && (
        <Stack>
          <FormControlLabel
            control={
              <Checkbox
                checked={pretty}
                onChange={e => {
                  setPretty(e.target.checked);
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

      {kind == "jsonline" && <></>}

      {kind == "html" && (
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
          loading={loading}
        >
          <ArrowCircleDownIcon> </ArrowCircleDownIcon>
        </IconButton>
        {loading && (
          <CancelButton
            handleCancel={() => {
              cancelTask(taskId.current, showNotification);
              exportedSize.current.innerText = "";
            }}
          />
        )}
        <Typography ref={exportedSize} />
        <OpenFileWithDefaultApp file={file} />
      </StackRowCenter>
    </Stack>
  );
}

function ExportTableOrView(props: {
  uuid: string;
  name: string;
  supportKind: string[];
  handleClose: () => void;
}) {
  const { uuid, name, supportKind, handleClose } = props;
  return (
    <ExportView
      exportOP={async (kind, filepath, taskId, sizeEmitter) => {
        return await invoke("export_table_or_view", {
          name: uuid,
          tableOrView: name,
          filepath,
          kind,
          sizeEmitter,
          taskId,
        });
      }}
      supportKind={supportKind}
      name={name}
      handleClose={handleClose}
    />
  );
}

export { ExportView, ExportTableOrView };
