import {
  IconButton,
  Stack,
  Tooltip,
  Typography,
  FormControlLabel,
  Checkbox,
} from "@mui/material";

import { PlayArrowIcon } from "./icons";
import { editor } from "monaco-editor";
import { useCallback, useContext, useEffect, useState } from "react";
import { MongoDBQuery } from "./MongoQueryBackend";
import * as monaco from "monaco-editor";

import {
  createMongoModel,
  DataSourceKind,
  isMacOs,
  jumpToEditorText,
  MonacoEditor,
  prettyMs,
  QueryIDEditorChatCtx,
  saveQueryKey,
  DbnameContext,
} from "./helper";
import { NameAndQueryId } from "./Common";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  LoadStoreFile,
  SaveFavoriteSqlView,
  ShowQueryLog,
  ToggleChatShow,
} from "./CommonComp2";
import {
  NotificationParams,
  useBoolSwitch,
  useNotification,
  useQueryLogStore,
} from "./store";
function removeSourceMapComment(code: string) {
  const target = "//# sourceMappingURL=";
  const lastIndex = code.lastIndexOf(target);

  if (lastIndex !== -1) {
    // 截取到注释开始之前的位置，并 trimDown 掉尾部多余换行
    return code.slice(0, lastIndex).trimEnd();
  }
  return code;
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
    let jsCode = emitOutput.outputFiles.find((f) => f.name.endsWith(".js"))?.text;
    const sourceMap = emitOutput.outputFiles.find((f) =>
      f.name.endsWith(".js.map"),
    )?.text;
    // 建议替代 dropLastLine 的方案
    jsCode = removeSourceMapComment(jsCode);
    return (
      jsCode +
      "\n" +
      `//# sourceURL=${uri.path}\n` +
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

const ignoreErrKey = "mongo-query-ignore-error";

function MongoQuery(props: { favoriteName?: string; value?: string }) {
  const { value, favoriteName } = props;
  const { dbName } = useContext(DbnameContext);
  const { showNotification } = useNotification();
  const { getSwitch, setSwitch } = useBoolSwitch();
  const [mongoHandle, setMongoHandle] = useState<{ db; FS; BSON; HTTP }>({
    db: new MongoDBQuery(dbName),
    BSON: null,
    FS: null,
    HTTP: null,
  });

  const [markdownModel, setMarkdownModel] = useState(false);

  const { save: saveQueryLog } = useQueryLogStore();

  const { editor, setEditor } = useContext(QueryIDEditorChatCtx);

  const onMount = useCallback(
    (ed: editor.IStandaloneCodeEditor) => {
      setEditor(ed);
      const model = createMongoModel(`//@ts-ignore
const { db, FS, HTTP, BSON } = mongoHandle;
`);
      ed.setModel(model);
      ed.getModel()._dbpal_query_id = new NameAndQueryId(
        dbName,
        DataSourceKind.Mongo,
        "",
      ); // mongo no need query id
      ed.focus();
    },
    [dbName, setEditor],
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
              if (getSwitch(saveQueryKey)) {
                saveQueryLog(dbName, editor.getValue(), true);
              }
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

          <SaveFavoriteSqlView editor={editor} favoriteName={favoriteName} />
          <LoadStoreFile
            getQuery={() => {
              return editor.getValue();
            }}
            kind={DataSourceKind.Mongo}
            handleLoadFile={(text) => {
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
          <ShowQueryLog />
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
              handleEexecute={() => {
                if (getSwitch(saveQueryKey)) {
                  saveQueryLog(dbName, code, true);
                }
                return handleExecute(code);
              }}
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

export { MongoQuery };
