import { Editor } from "@monaco-editor/react";
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
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { initMongoQueryGlobals, MongoDBQuery } from "./MongoQueryBackend";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  DataSourceKind,
  ErrAndLoc,
  generateRandomString,
  isMacOs,
  isWindows,
  MongoLanguage,
  QueryLogHandler,
  saveQueryLog,
  secondConfirm,
  SettingContext,
  validJson,
} from "./helper";
import { EscDownEvent, modalHandleClose, modalStyle } from "./Common";
import { NotificationContext } from "./Notification";
import { ModalHeader } from "./CommonComp";
import { RiDeleteBack2Line } from "react-icons/ri";
import {
  AppMarkDown,
  EditorBar,
  LoadStoreFile,
  SaveFavoriteSqlView,
  SimpleFlexEditor,
  StackRowCenter,
  syntaxHightLightTheme,
  ToggleChatShow,
} from "./CommonComp2";
import { QueryLog } from "./QueryLog";
import InfoIcon from "@mui/icons-material/Info";
import intervalCleanTest from "../../tests/interval_clean_test.js?raw";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { tempDir, join } from "@tauri-apps/api/path";
import { exists, mkdir, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { ChildProcess, Command } from "@tauri-apps/plugin-shell";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
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

class SaveTsOption {
  static key = "dbpal-ts-compiler-option";

  static getStr() {
    try {
      const op = localStorage.getItem(SaveTsOption.key);
      if (op) {
        return op;
      }
    } catch (e) {
      /* empty */
      console.log("get config failed", e);
    }
    return `{
        "target": "esnext",
        "lib": [
            "esnext",
            "dom"
        ]
}`;
  }
  static get(): object {
    return JSON.parse(SaveTsOption.getStr());
  }
  static set(x: object) {
    localStorage.setItem(SaveTsOption.key, JSON.stringify(x, null, 2));
  }
}

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

  async compile(value: string, sourceMap: boolean): Promise<CompilerOutput> {
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
    const compilerOP = SaveTsOption.get();
    // make sure compilerOptions.outDir

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
  const [value, setValue] = useState(SaveTsOption.getStr());
  const { showNotification } = useContext(NotificationContext);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  useEffect(() => {
    (async () => {
      const x = await validJson(value);
      setJsonErr(x);
    })();
  }, [value]);

  const handleSave = () => {
    try {
      SaveTsOption.set(JSON.parse(value));
    } catch (e) {
      showNotification("error", "save option failed", e);
      return;
    }
    showNotification("success", "save option success");
    handleClose();
  };

  return (
    <Stack height="100%">
      <SimpleFlexEditor
        value={value}
        language="json"
        handleValue={(value) => {
          setValue(value);
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
              }}
            >
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
          }}
        >
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
        }}
      >
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
  const { showNotification } = useContext(NotificationContext);
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
        showNotification("error", "is typescript  global installed?", e);
      });
  }, [language, showNotification]);

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
              );
              if (result.out.code != 0) {
                showNotification(
                  "error",
                  "typescript compile error,can safe change to javascript",
                  <TsCompilerMsg out={result.out} editor={editor} />,
                );
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
            }}
          >
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
          title=<CompilerNotice width="40vw" compileDir={compileDir} />
        >
          <Typography> compile </Typography>
        </Tooltip>
      )}
      {showEditOption && (
        <Modal
          open={showEditOption}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditOption(false);
          })}
        >
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
            }}
          >
            {line.msg}
          </Typography>
        );
      })}{" "}
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
            }}
          >
            {x.name}
          </Button>
        ))}
      </Stack>
    </Paper>
  );
}
function MongoQuery(props: {
  uuid: string;
  favoriteName?: string;
  value?: string;
  language?: string;
  handleEditor: (x: editor.IStandaloneCodeEditor) => void;
}) {
  const { uuid, value, handleEditor, language: valueLanguage, favoriteName } = props;
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [executeTimes, setExecuteTimes] = useState(0);
  const setting = useContext(SettingContext);
  const [db] = useState(new MongoDBQuery(uuid));
  const [cleanFns, setCleanFns] = useState<CleanFunction[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [bson, setBson] = useState(null);
  const [language, setLanguage] = useState<MongoLanguage>(
    valueLanguage ? (valueLanguage as MongoLanguage) : lastLanguage,
  );
  const [compileTime, setCompileTime] = useState<number | null>(null);
  const compileDir = useRef(generateRandomString(5));
  useEffect(() => {
    lastLanguage = language;
  }, [language]);

  const setUpClean = (name: string, fn: () => void) => {
    setCleanFns((e) => [...e, { name, fn }]);
  };
  const jsCodeMount = useCallback((ed: editor.IStandaloneCodeEditor) => {
    setEditor(ed);
    ed.focus();
    let d1 = ed.onDidChangeCursorPosition(() => {
      setPos(ed.getPosition());
    });
    return () => {
      d1.dispose();
    };
  }, []);
  useEffect(() => {
    if (!editor) {
      return;
    }
    handleEditor(editor);
  }, [editor, handleEditor]);

  useEffect(() => {
    import("bson").then((e) => {
      setBson(e);
    });
  }, []);

  const handleExecute = async () => {
    let code = editor.getValue();
    if (!code.trim()) {
      showNotification("warning", "can't execute empty statement");
      return;
    }
    if (saveQueryLog.get()) {
      new QueryLogHandler(uuid).save(code, true, language);
    }
    if (language == MongoLanguage.ts) {
      let out: CompilerOutput;
      try {
        out = await new TSCompiler(compileDir.current).compile(code, true);
      } catch (e) {
        console.error("compile failed", e);
      }
      setCompileTime(out.compileTime);
      if (out.out.code != 0) {
        showNotification(
          "error",
          "compile failed",
          <TsCompilerMsg out={out.out} editor={editor} />,
        );
        return;
      }
      code = out.jsCode;
    }
    setExecuteTimes((e) => e + 1);
    eval(code);
    showNotification(
      "success",
      "execute ok,but somehow async function or timeout function ... etc may running in background",
    );
  };

  const options = useMemo((): editor.IStandaloneEditorConstructionOptions => {
    return {
      fontSize: setting.appearance.fontSize,
      fontWeight: setting.appearance.fontWeight,
      fontFamily: setting.appearance.fontFamily,
      tabSize: setting.editor.tabSize,
      ...setting.editor.editorOption,
    };
  }, [setting]);

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
        sx={{ alignItems: "center", overflow: "auto", paddingBottom: "0.5rem" }}
      >
        <TypeScript
          language={language}
          setLanguage={setLanguage}
          editor={editor}
          compileTime={compileTime}
          compileDir={compileDir.current}
        />
        <Tooltip title="execute">
          <IconButton
            onClick={() => {
              handleExecute();
            }}
            color="primary"
          >
            <PlayArrowIcon />
          </IconButton>
        </Tooltip>
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
            color="primary"
          >
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
            }}
          >
            <RiDeleteBack2Line fontSize="1.6rem" />
          </Tooltip>
        )}

        <Tooltip
          title=<CleanFunctionDoc />
          slotProps={{
            tooltip: {
              style: { maxWidth: "60vw" },
            },
          }}
        >
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
          }}
        >
          <span>debug???</span>
        </Tooltip>

        <ToggleChatShow do_query={handleExecute} />
      </Stack>
      <div
        style={{
          height: "100px",
          flexGrow: 1,
          flexShrink: 1,
          flexBasis: 0,
        }}
      >
        <Editor
          width="99%"
          height="99%"
          onMount={jsCodeMount}
          language={language}
          options={options}
          value={value}
          theme={setting.editorTheme()}
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
      <SyntaxHighlighter
        style={syntaxHightLightTheme()} // Apply the chosen theme
        language={"javascript"}
        PreTag="div"
      >
        {intervalCleanTest}
      </SyntaxHighlighter>{" "}
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
