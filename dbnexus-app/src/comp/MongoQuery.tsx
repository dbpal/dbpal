import {
  Autocomplete,
  Box,
  Button,
  Chip,
  IconButton,
  Link,
  Modal,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";

import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { editor, Position } from "monaco-editor";
import { useCallback, useContext, useEffect, useRef, useState } from "react";
import { initMongoQueryGlobals, MongoDBQuery } from "./MongoQueryBackend";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  DataSourceKind,
  ErrAndLoc,
  generateRandomString,
  getEditorText,
  isMacOs,
  isWindows,
  jumpToEditorText,
  MonacoEditor,
  MongoLanguage,
  QueryIDEditorChatCtx,
  saveQueryKey,
  secondConfirm,
  validJson,
} from "./helper";
import {
  dbpalQuerykey,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
} from "./Common";
import { ModalHeader } from "./CommonComp";
import { RiDeleteBack2Line } from "react-icons/ri";
import {
  AiMarkdownModel,
  AiMarkdownModelSwitch,
  AppCode,
  AppMarkDown,
  EditorBar,
  LoadStoreFile,
  SaveFavoriteSqlView,
  SimpleFlexEditor,
  StackRowCenter,
  ToggleChatShow,
} from "./CommonComp2";
import { QueryLog } from "./QueryLog";
import InfoIcon from "@mui/icons-material/Info";
import intervalCleanTest from "../../tests/interval_clean_test.js?raw";
import { tempDir, join } from "@tauri-apps/api/path";
import { exists, mkdir, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { ChildProcess, Command } from "@tauri-apps/plugin-shell";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { useBoolSwitch, useNotification, useQueryLogStore } from "./store";
initMongoQueryGlobals();

class CompilerOutput {
  constructor(
    public out: ChildProcess<string>,
    public compileTime: number,
    public jsCode?: string,
  ) {}
}

function dropLastLine(str) {
  return str.split(/\r?\n/).slice(0, -1).join("\n");
}

import { create } from "zustand";
import { persist } from "zustand/middleware";

// 定義初始的預設配置字串（轉為物件）
const DEFAULT_COMPILER_OPTIONS = {
  target: "esnext",
  lib: ["esnext", "dom"],
};

interface TsOptionState {
  // 存放編譯選項物件
  options: object;

  // 更新選項
  setOptions: (newOptions: object) => void;

  // 重置回預設值
  resetOptions: () => void;
}

export const useTsOptionStore = create<TsOptionState>()(
  persist(
    (set) => ({
      // 初始狀態
      options: DEFAULT_COMPILER_OPTIONS,

      setOptions: (newOptions) => set({ options: newOptions }),

      resetOptions: () => set({ options: DEFAULT_COMPILER_OPTIONS }),
    }),
    {
      name: "dbpal-ts-compiler-option",
    },
  ),
);

class TSCompiler {
  constructor(public compileDir: string) {}
  async getRootDir(): Promise<string> {
    const data_dir = await tempDir();
    const rootPath = await join(
      data_dir,
      "dbpal-mongo-query-ts-compile",
      this.compileDir,
    );
    if (!(await exists(rootPath))) {
      await mkdir(rootPath, { recursive: true });
    }
    return rootPath;
  }

  async compile(
    value: string,
    sourceMap: boolean,
    compilerOP: object,
  ): Promise<CompilerOutput> {
    // dir
    const rootPath = await this.getRootDir();

    // write main.ts
    const mainTsFilePath = await join(rootPath, "main.ts");
    await writeTextFile(mainTsFilePath, value, {
      create: true,
      append: false,
    });

    // write config file
    const optionFilePath = await join(rootPath, "tsconfig.json");
    compilerOP["outDir"] = "./dist";
    compilerOP["sourceMap"] = sourceMap;
    compilerOP["typeRoots"] = ["./types"];
    await writeTextFile(
      optionFilePath,
      JSON.stringify({ compilerOptions: compilerOP }, null, 2),
      { create: true, append: false },
    );
    const typesPath = await join(rootPath, "types");

    if (!(await exists(typesPath))) {
      await mkdir(typesPath);
    }
    const extraTypeFile = await join(typesPath, "extra.ts");
    if (!(await exists(extraTypeFile))) {
      await writeTextFile(
        extraTypeFile,
        `
      let db : any;
      let cleanFns : (()=> void)[];
      let bson : any;
      `,
      );
    }
    const start = new Date();
    const cmd = Command.create(TSCompiler.getCompilerCmd(), [], {
      cwd: rootPath,
    });
    const out = await cmd.execute();
    const compileTime = new Date().getTime() - start.getTime();
    if (out.code !== 0) {
      return new CompilerOutput(out, compileTime);
    }
    const code = await readTextFile(await join(rootPath, "dist", "main.js"));
    if (!sourceMap) {
      return new CompilerOutput(out, compileTime, code);
    }
    const sourceMapText = await readTextFile(
      await join(rootPath, "dist", "main.js.map"),
    );
    let codeWithMap = dropLastLine(code);
    codeWithMap +=
      "//# sourceMappingURL=data:application/json;charset=utf-8;base64," +
      btoa(sourceMapText);
    return new CompilerOutput(out, compileTime, codeWithMap);
  }
  static async getTscVersion(): Promise<string> {
    const cmd = Command.create(TSCompiler.getCompilerCmd(), ["--version"]);
    const out = await cmd.execute();
    return out.stdout.substring(out.stdout.indexOf(" ") + 1);
  }

  static getCompilerCmd() {
    return isWindows() ? "run-tsc-cmd" : "run-tsc";
  }
}

function TsEditCompilerOptions(props: { handleClose: () => void }) {
  const { handleClose } = props;
  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);

  const { options, setOptions } = useTsOptionStore();
  const { showNotification } = useNotification();
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);

  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const handleSave = () => {
    try {
      let x = JSON.parse(editor.getValue());
      setOptions(x);
    } catch (e) {
      showNotification({ severity: "error", title: "save option failed", message: e });
      return;
    }
    showNotification({ severity: "success", title: "save option success" });
    handleClose();
  };

  useEffect(() => {
    if (!editor) {
      return;
    }

    editor.onDidChangeModel(() => {
      validJson(editor.getValue()).then((x) => {
        setJsonErr(x);
      });
    });
  }, [editor]);

  return (
    <Stack height="100%">
      <SimpleFlexEditor
        value={JSON.stringify(options, null, 2)}
        language="json"
        handleEditor={(ed) => {
          setEditor(ed);
        }}
        errPos={jsonErrPos}
        barLeft={
          jsonErr && (
            <Typography
              color="error"
              style={{ cursor: "pointer" }}
              onClick={() => {
                if (!jsonErr) {
                  return;
                }
                setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
              }}>
              {jsonErr.msg}
            </Typography>
          )
        }
      />
      <StackRowCenter>
        <Button onClick={handleSave}> save </Button>
        <Link
          href="#"
          underline="hover"
          onClick={(e) => {
            e.preventDefault();
            openUrl("https://www.typescriptlang.org/tsconfig/#compilerOptions");
          }}>
          options
        </Link>
      </StackRowCenter>
    </Stack>
  );
}

function CompilerNotice(props: { compileDir: string; width: string }) {
  const { compileDir, width } = props;

  const [rootPath, setRootPath] = useState("");
  useEffect(() => {
    new TSCompiler(compileDir).getRootDir().then((path) => {
      setRootPath(path);
    });
  }, [compileDir]);

  return (
    <Stack style={{ width, padding: "0.5rem" }}>
      <AppMarkDown
        handlePathUrl={(p) => {
          openPath(p);
        }}>
        {`### typescript compile notice 
  
1. we use global installed typescript compiler.
2. we copy the code to the [temp dir](${rootPath}).
3. set up tsconfig.json.
4. call command 'tsc'.
5. retriver the compiled code from filesystem.

          `}
      </AppMarkDown>
    </Stack>
  );
}

function TypeScript(props: {
  language: string;
  setLanguage;
  editor: editor.IStandaloneCodeEditor;
  compileTime?: number;
  compileDir: string;
}) {
  const { language, setLanguage, editor, compileTime, compileDir } = props;
  const [showEditOption, setShowEditOption] = useState(false);
  const { showNotification } = useNotification();
  const [tscVersion, setTscVersion] = useState("");
  useEffect(() => {
    if (language == MongoLanguage.js) {
      return;
    }
    TSCompiler.getTscVersion()
      .then((v) => {
        setTscVersion(v);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "is typescript  global installed?",
          message: e,
        });
      });
  }, [language, showNotification]);

  const options = useTsOptionStore((s) => s.options);
  return (
    <>
      <Autocomplete
        options={[MongoLanguage.js, MongoLanguage.ts]}
        value={language}
        style={{ width: "12rem" }}
        renderInput={(params) => <TextField {...params} label={"language"} />}
        freeSolo={false}
        onChange={(_, name) => {
          (async () => {
            if (name == MongoLanguage.js) {
              const confirm = await secondConfirm({
                title: "Warning",
                content: "Switching to javascript may lose some type definitions",
                kind: "warning",
              });
              if (!confirm) {
                return;
              }
              const result = await new TSCompiler(compileDir).compile(
                editor.getValue(),
                false,
                options,
              );
              if (result.out.code != 0) {
                showNotification({
                  severity: "error",
                  title: "typescript compile error,can safe change to javascript",
                  message: <TsCompilerMsg out={result.out} editor={editor} />,
                });
                return;
              }
              editor.setValue(result.jsCode);
            }
            setLanguage(name);
          })();
        }}
      />

      {language == MongoLanguage.ts && tscVersion && (
        <Tooltip title="typescript version">
          <Chip color="success" label={tscVersion} />
        </Tooltip>
      )}
      {language == MongoLanguage.ts && (
        <Tooltip title="edit compiler option">
          <Button
            variant="outlined"
            onClick={() => {
              setShowEditOption(true);
            }}>
            option
          </Button>
        </Tooltip>
      )}
      {language == MongoLanguage.ts && compileTime && (
        <Tooltip title="compile time">
          <Chip label={`${compileTime}ms`} color="info" />
        </Tooltip>
      )}
      {language == MongoLanguage.ts && (
        <Tooltip
          slotProps={{
            tooltip: {
              style: { maxWidth: "40vw" },
            },
          }}
          title=<CompilerNotice width="40vw" compileDir={compileDir} />>
          <Typography> compile </Typography>
        </Tooltip>
      )}
      {showEditOption && (
        <Modal
          disableAutoFocus
          disableEnforceFocus
          open={showEditOption}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditOption(false);
          })}>
          <Box sx={modalStyle} width="75%" height="65vh">
            <ModalHeader
              title="edit compiler option"
              handleClose={() => {
                setShowEditOption(false);
              }}
              noTextTransform
            />
            <TsEditCompilerOptions
              handleClose={() => {
                setShowEditOption(false);
              }}
            />
          </Box>
        </Modal>
      )}
    </>
  );
}

let lastLanguage = MongoLanguage.js;

function TsCompilerMsg(props: {
  editor: editor.IStandaloneCodeEditor;
  out: ChildProcess<string>;
}) {
  const { editor, out } = props;
  const [lines, setLines] = useState<
    {
      msg: string;
      pos: Position;
    }[]
  >([]);

  useEffect(() => {
    const lines = (out.stdout + out.stderr).split("\n");
    setLines(
      lines.map((x) => {
        const pos = extractLineAndColumn(x);
        return { msg: x, pos };
      }),
    );
  }, [out]);
  return (
    <Stack>
      {lines.map((line, index) => {
        return (
          <Typography
            key={index}
            color={out.code == 0 ? "success" : "error"}
            style={{ cursor: line.pos ? "pointer" : undefined }}
            onClick={() => {
              editor.setPosition(line.pos);
              editor.focus();
            }}>
            {line.msg}
          </Typography>
        );
      })}
    </Stack>
  );
}

interface CleanFunction {
  name: string;
  fn: () => void;
}

function CallCleanFunctions(props: {
  cleanFunctions: CleanFunction[];
  setCleanFunction;
  width: string;
}) {
  const { cleanFunctions, setCleanFunction, width } = props;
  const handleCall = (index: number) => {
    cleanFunctions[index].fn();
    setCleanFunction(cleanFunctions.filter((_x, i) => index != i));
  };

  return (
    <Paper style={{ width }}>
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
  getText: () => string;
  disabled: boolean;
}) {
  const { handleEexecute, getText, disabled } = props;

  return (
    <Tooltip title="execute">
      <IconButton
        onClick={() => {
          const text = getText();
          if (!text) {
            return;
          }
          handleEexecute(text);
        }}
        disabled={disabled}
        color="primary">
        <PlayArrowIcon />
      </IconButton>
    </Tooltip>
  );
}
function MongoQuery(props: {
  uuid: string;
  favoriteName?: string;
  value?: string;
  language?: string;
}) {
  const { uuid, value, language: valueLanguage, favoriteName } = props;
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [executeTimes, setExecuteTimes] = useState(0);
  const [db] = useState(new MongoDBQuery(uuid));
  const [cleanFns, setCleanFns] = useState<CleanFunction[]>([]);
  const { showNotification } = useNotification();
  const [bson, setBson] = useState(null);
  const [language, setLanguage] = useState<MongoLanguage>(
    valueLanguage ? (valueLanguage as MongoLanguage) : lastLanguage,
  );
  const [markdownModel, setMarkdownModel] = useState(false);
  const [markdownModelValue, setMarkdownModelValue] = useState("");
  const [compileTime, setCompileTime] = useState<number | null>(null);
  const compileDir = useRef(generateRandomString(5));
  useEffect(() => {
    lastLanguage = language;
  }, [language]);
  const { save: saveQueryLog } = useQueryLogStore();

  const setEditorContext = useContext(QueryIDEditorChatCtx);
  useEffect(() => {
    if (!editor) {
      return;
    }
    setEditorContext.setEditor(editor);
  }, [editor, setEditor, setEditorContext]);

  const setUpClean = (name: string, fn: () => void) => {
    setCleanFns((e) => [...e, { name, fn }]);
  };
  const onMount = useCallback(
    (ed: editor.IStandaloneCodeEditor) => {
      setEditor(ed);
      ed.getModel()[dbpalQuerykey] = new NameAndQueryId(uuid, DataSourceKind.Mongo, ""); // mongo no need query id
      ed.focus();
      let d1 = ed.onDidChangeCursorPosition(() => {
        setPos(ed.getPosition());
      });
      return () => {
        d1.dispose();
      };
    },
    [uuid],
  );

  useEffect(() => {
    import("bson").then((e) => {
      setBson(e);
    });
  }, []);

  const { getSwitch } = useBoolSwitch();
  const compilerOp = useTsOptionStore((s) => s.options);

  const handleExecute = async (code: string) => {
    if (getSwitch(saveQueryKey)) {
      saveQueryLog(uuid, editor.getValue(), true, language);
    }
    if (language == MongoLanguage.ts) {
      let out: CompilerOutput;
      try {
        out = await new TSCompiler(compileDir.current).compile(code, true, compilerOp);
      } catch (e) {
        console.error("compile failed", e);
      }
      setCompileTime(out.compileTime);
      if (out.out.code != 0) {
        showNotification({
          severity: "error",
          title: "compile failed",
          message: <TsCompilerMsg out={out.out} editor={editor} />,
        });
        return;
      }
      code = out.jsCode;
    }
    setExecuteTimes((e) => e + 1);
    eval(code);
    showNotification({
      severity: "success",
      title:
        "execute ok,but somehow async function or timeout function ... etc may running in background",
    });
  };
  return (
    <Stack style={{ height: "100%" }}>
      <QueryLog
        showQueryLog={showQueryLog}
        setShowQueryLog={setShowQueryLog}
        uuid={uuid}
        kind={DataSourceKind.Mongo}
        executeTimes={executeTimes}
      />
      <Stack
        direction="row"
        sx={{ alignItems: "center", overflow: "auto", paddingBottom: "0.5rem" }}>
        <ExecuteButton
          disabled={markdownModel}
          getText={() => {
            let code = getEditorText(editor, showNotification);
            if (!code.trim()) {
              showNotification({
                severity: "warning",
                title: "can't execute empty statement",
              });
              return;
            }
            return code;
          }}
          handleEexecute={handleExecute}
        />
        <TypeScript
          language={language}
          setLanguage={setLanguage}
          editor={editor}
          compileTime={compileTime}
          compileDir={compileDir.current}
        />
        <SaveFavoriteSqlView
          language={language}
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
          handleLoadFile={(path, text) => {
            if (path.endsWith(".ts")) {
              setLanguage(MongoLanguage.ts);
            }
            editor.setValue(text);
          }}
        />
        <Tooltip title="query log">
          <IconButton
            onClick={() => {
              setShowQueryLog((prev) => !prev);
            }}
            color="primary">
            <EventNoteIcon />
          </IconButton>
        </Tooltip>
        {cleanFns.length > 0 && (
          <Tooltip
            title=<CallCleanFunctions
              cleanFunctions={cleanFns}
              setCleanFunction={setCleanFns}
              width="45vw"
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
          title=<Stack width="40vw">
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
        <ToggleChatShow />
        <AiMarkdownModelSwitch
          setMarkdownModel={setMarkdownModel}
          markdownModel={markdownModel}
        />
      </Stack>

      <AiMarkdownModel
        markdownModel={markdownModel}
        setMarkdownModelValue={setMarkdownModelValue}
        editor={editor}
        markdownModelValue={markdownModelValue}
        appCodeHandleEdit={({ code }) => {
          setMarkdownModel(false);
          jumpToEditorText(editor, code);
        }}
        appcodeExtra={({ code }) => {
          return (
            <ExecuteButton
              getText={() => {
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
          language={markdownModel ? "markdown" : language}
          value={value}
        />
        <EditorBar pos={pos} />
      </div>
      {/* // we must use db etc ...  */}
      {db && <span />}
      {cleanFns && <span />}

      {bson && <span />}
      {setUpClean && <span />}
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

// main.ts(2,1): error TS2364: The left-hand side of an assignment expression must be a variable or a property access.
// extract line and column from error message
function extractLineAndColumn(text: string): Position | undefined {
  if (!text.startsWith("main.ts")) {
    return undefined;
  }
  const sub = text.substring(text.indexOf("(") + 1, text.indexOf(")"));
  const arr = sub.split(",");
  return new Position(parseInt(arr[0]), parseInt(arr[1]));
}

export { MongoQuery };
