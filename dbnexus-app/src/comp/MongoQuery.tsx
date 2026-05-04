import {
  Button,
  IconButton,
  Paper,
  Stack,
  Tooltip,
  Typography,
  SxProps,
  Theme,
  FormControlLabel,
  Checkbox,
} from "@mui/material";

import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { editor } from "monaco-editor";
import { useCallback, useContext, useEffect, useState } from "react";
import { MongoDBQuery } from "./MongoQueryBackend";
import * as monaco from "monaco-editor";

import {
  DataSourceKind,
  isMacOs,
  jumpToEditorText,
  MonacoEditor,
  prettyMs,
  QueryIDEditorChatCtx,
  saveQueryKey,
} from "./helper";
import { dbpalQuerykey, NameAndQueryId } from "./Common";
import { RiDeleteBack2Line } from "react-icons/ri";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  AppCode,
  LoadStoreFile,
  SaveFavoriteSqlView,
  ShowQueryLog,
  ToggleChatShow,
} from "./CommonComp2";
import InfoIcon from "@mui/icons-material/Info";
import intervalCleanTest from "../../tests/interval_clean_test.js?raw";
import {
  NotificationParams,
  useBoolSwitch,
  useNotification,
  useQueryLogStore,
} from "./store";

function dropLastLine(str: string) {
  return str.split(/\r?\n/).slice(0, -1).join("\n");
}

monaco.typescript.typescriptDefaults.setCompilerOptions({
  sourceMap: true,
  // 注意：ScriptTarget 和 ModuleKind 也在 typescript 下
  target: monaco.typescript.ScriptTarget.ESNext,
  module: monaco.typescript.ModuleKind.ESNext,
  noEmit: false,
});
async function transformTS(
  editorIns: editor.IStandaloneCodeEditor,
  showNotification: (_: NotificationParams) => void,
  ignoreErr: boolean,
) {
  try {
    const model = editorIns.getModel();
    const uri = model.uri;
    const uriStr = uri.toString();

    // 1. 获取 TS Worker
    const workerGetter = await monaco.typescript.getTypeScriptWorker();
    const worker = await workerGetter(uri);
    if (!ignoreErr) {
      const diagnostics = await Promise.all([
        worker.getSyntacticDiagnostics(uriStr),
        worker.getSemanticDiagnostics(uriStr),
      ]);

      let allDiagnostics = diagnostics.flat();

      allDiagnostics = allDiagnostics.filter(
        (x) => x.category == 1,
        // 1 means  error
      );
      if (allDiagnostics.length > 0) {
        for (const x of allDiagnostics) {
          showNotification({
            severity: "error",
            message: (
              <Typography
                style={{ cursor: "pointer" }}
                onClick={() => {
                  const pos = model.getPositionAt(x.start);
                  editorIns.setPosition(pos);
                  editorIns.focus();
                }}>
                {(function () {
                  if (typeof x.messageText == "string") {
                    return x.messageText;
                  } else {
                    return x.messageText.messageText;
                  }
                })()}
              </Typography>
            ),
          });
        }

        return "";
      }
    }

    // 2. 调用编译方法 (getEmitOutput 包含 js 和 map)
    const emitOutput = await worker.getEmitOutput(uri.toString());

    // 3. 提取结果
    // outputFiles 通常包含两个文件：一个 .js，一个 .js.map
    const jsCode = emitOutput.outputFiles.find((f) => f.name.endsWith(".js"))?.text;
    const sourceMap = emitOutput.outputFiles.find((f) =>
      f.name.endsWith(".js.map"),
    )?.text;

    return (
      dropLastLine(jsCode) +
      "//# sourceMappingURL=data:application/json;charset=utf-8;base64," +
      btoa(sourceMap)
    );
  } catch (e) {
    showNotification({
      severity: "error",
      title: "transpile typescript failed",
      message: e,
    });
    return "";
  }
}

interface CleanFunction {
  name: string;
  fn: () => void;
}

function CallCleanFunctions(props: {
  cleanFunctions: CleanFunction[];
  setCleanFunction;
  sx?: SxProps<Theme>;
}) {
  const { cleanFunctions, setCleanFunction, sx } = props;
  const handleCall = (index: number) => {
    cleanFunctions[index].fn();
    setCleanFunction(cleanFunctions.filter((_x, i) => index != i));
  };

  return (
    <Paper sx={sx}>
      <Stack>
        {cleanFunctions.map((x, index) => (
          <Button
            key={index}
            variant="outlined"
            onClick={() => {
              handleCall(index);
            }}>
            {x.name}
          </Button>
        ))}
      </Stack>
    </Paper>
  );
}

function ExecuteButton(props: {
  handleEexecute: (text: string) => void;
  getText: () => Promise<string>;
  disabled: boolean;
}) {
  const { handleEexecute, getText, disabled } = props;

  return (
    <Tooltip title="execute">
      <span>
        <IconButton
          onClick={async () => {
            const text = await getText();
            if (!text) {
              return;
            }
            handleEexecute(text);
          }}
          disabled={disabled}
          color="primary">
          <PlayArrowIcon />
        </IconButton>
      </span>
    </Tooltip>
  );
}

let modelCounter = 0;

function createNewModel(code = "") {
  modelCounter++;
  const uri = monaco.Uri.parse(`file:///model_${modelCounter}.ts`);

  return monaco.editor.createModel(code, "typescript", uri);
}

const ignoreErrKey = "mongo-query-ignore-error";

function MongoQuery(props: { uuid: string; favoriteName?: string; value?: string }) {
  const { uuid, value, favoriteName } = props;
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);

  const [cleanFns, setCleanFns] = useState<CleanFunction[]>([]);
  const { showNotification } = useNotification();
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [mongoHandle, setMongoHandle] = useState<{ db; setUpClean; FS; BSON; HTTP }>({
    db: new MongoDBQuery(uuid),
    setUpClean: (name: string, fn: () => void) => {
      setCleanFns((e) => [...e, { name, fn }]);
    },
    BSON: null,
    FS: null,
    HTTP: null,
  });

  const [markdownModel, setMarkdownModel] = useState(false);

  const { save: saveQueryLog } = useQueryLogStore();

  const setEditorContext = useContext(QueryIDEditorChatCtx);
  useEffect(() => {
    if (!editor) {
      return;
    }
    setEditorContext.setEditor(editor);
  }, [editor, setEditor, setEditorContext]);

  const onMount = useCallback(
    (ed: editor.IStandaloneCodeEditor) => {
      setEditor(ed);
      const model = createNewModel(`//@ts-ignore
const { db, FS, HTTP, BSON } = mongoHandle;
`);
      ed.setModel(model);
      ed.getModel()[dbpalQuerykey] = new NameAndQueryId(uuid, DataSourceKind.Mongo, ""); // mongo no need query id
      ed.focus();
    },
    [uuid],
  );

  useEffect(() => {
    import("bson").then((e) => {
      setMongoHandle((mongoHandle) => ({ ...mongoHandle, BSON: e }));
    });
    import("@tauri-apps/plugin-fs").then((e) => {
      setMongoHandle((mongoHandle) => ({ ...mongoHandle, FS: e }));
    });
    import("@tauri-apps/plugin-http").then((e) => {
      setMongoHandle((mongoHandle) => ({ ...mongoHandle, HTTP: e }));
    });
  }, []);

  const handleExecute = async (code: string) => {
    if (getSwitch(saveQueryKey)) {
      saveQueryLog(uuid, code, true);
    }
    eval(code);
    showNotification({
      severity: "success",
      title:
        "execute ok,but somehow async function or timeout function ... etc may running in background",
    });
  };
  return (
    <Stack style={{ height: "100%" }}>
      <Stack
        direction="row"
        sx={{
          alignItems: "center",
          overflow: "auto",
          paddingBottom: "0.5rem",
          justifyContent: "space-between",
        }}>
        <Stack direction="row" style={{ alignItems: "center" }}>
          <ExecuteButton
            disabled={markdownModel}
            getText={async () => {
              let now = new Date();
              const code = await transformTS(
                editor,
                showNotification,
                getSwitch(ignoreErrKey),
              );
              if (code) {
                showNotification({
                  severity: "info",
                  title: `transpile typescript used ${prettyMs(new Date().getTime() - now.getTime())}`,
                });
              }
              return code;
            }}
            handleEexecute={handleExecute}
          />

          <SaveFavoriteSqlView
            kind={DataSourceKind.Mongo}
            editor={editor}
            uuid={uuid}
            favoriteName={favoriteName}
          />
          <LoadStoreFile
            getQuery={() => {
              return editor.getValue();
            }}
            kind={DataSourceKind.Mongo}
            handleLoadFile={(_path, text) => {
              editor.setValue(text);
            }}
          />
          <Tooltip title="try transpile to javascript ignore the errors.">
            <FormControlLabel
              control={
                <Checkbox
                  checked={getSwitch(ignoreErrKey)}
                  onChange={(e) => setSwitch(ignoreErrKey, e.target.checked)}
                />
              }
              label="ignore error"
            />
          </Tooltip>
        </Stack>

        <Stack direction="row" style={{ alignItems: "center" }}>
          <ToggleChatShow />
          <AiMarkdownModelSwitch
            setMarkdownModel={setMarkdownModel}
            markdownModel={markdownModel}
          />
          <ShowQueryLog uuid={uuid} kind={DataSourceKind.Mongo} />
          {cleanFns.length > 0 && (
            <Tooltip
              title=<CallCleanFunctions
                cleanFunctions={cleanFns}
                setCleanFunction={setCleanFns}
                sx={{ width: "45vw" }}
              />
              slotProps={{
                tooltip: {
                  style: { maxWidth: "45vw" },
                },
              }}>
              <RiDeleteBack2Line fontSize="1.6rem" />
            </Tooltip>
          )}
          <Tooltip
            title=<CleanFunctionDoc />
            slotProps={{
              tooltip: {
                style: { maxWidth: "60vw" },
              },
            }}>
            <InfoIcon> </InfoIcon>
          </Tooltip>
          <Tooltip
            title=<Stack sx={{ width: "40vw" }}>
              <Typography color="info ">
                we are still using `console` to debug.
              </Typography>
              <Typography>
                open dev tools:
                {isMacOs() ? "Command+Option+i" : "Ctrl+Shift+i"}
              </Typography>
            </Stack>
            slotProps={{
              tooltip: {
                style: { maxWidth: "40vw" },
              },
            }}>
            <span>debug???</span>
          </Tooltip>
        </Stack>
      </Stack>
      <AiMarkdownModel
        markdownModel={markdownModel}
        editor={editor}
        appCodeHandleEdit={({ code }) => {
          setMarkdownModel(false);
          jumpToEditorText(editor, code);
        }}
        appcodeExtra={({ code }) => {
          return (
            <ExecuteButton
              getText={async () => {
                return code;
              }}
              handleEexecute={handleExecute}
              disabled={false}
            />
          );
        }}
      />
      <div
        style={{
          height: "100px",
          flexGrow: 1,
          flexShrink: 1,
          flexBasis: 0,
          display: markdownModel ? "none" : "block",
        }}>
        <MonacoEditor
          onMount={onMount}
          language={markdownModel ? "markdown" : "typescript"}
          value={value}
        />
      </div>
      {/* // we must use db etc ...  */}

      {mongoHandle && <span />}
    </Stack>
  );
}

function CleanFunctionDoc() {
  return (
    <Paper style={{ maxWidth: 900, overflow: "auto" }}>
      <AppCode language={"javascript"} codeBlock={intervalCleanTest} />
    </Paper>
  );
}

export { MongoQuery };
