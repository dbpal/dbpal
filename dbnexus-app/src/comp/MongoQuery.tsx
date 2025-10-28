"use strict";
import { Editor, Monaco } from "@monaco-editor/react";
import {
  Autocomplete,
  Box,
  Button,
  Chip,
  IconButton,
  Link,
  Modal,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";

import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { editor, Position } from "monaco-editor";
import { useContext, useEffect, useMemo, useState } from "react";
import { initMongoQueryGlobals, MongoDBQuery } from "./MongoQueryBackend";
import EventNoteIcon from "@mui/icons-material/EventNote";
import {
  DataSourceKind,
  ErrAndLoc,
  isMacOs,
  isWindows,
  QueryLogHandler,
  SaveQueryLogConfigure,
  secondConfirm,
  SettingContext,
  validJson,
} from "./helper";
import { EscDownEvent, modalHandleClose, modalStyle } from "./Common";
import { NotificationContext } from "./Notification";
import { ModalHeader } from "./CommonComp";
import { FaBookmark } from "react-icons/fa";
import {
  EditorBar,
  LoadStoreFile,
  SimpleFlexEditor,
  syntaxHightLightTheme,
} from "./CommonComp2";
import { QueryLog } from "./QueryLog";
import InfoIcon from "@mui/icons-material/Info";
import intervalCleanTest from "../../tests/interval_clean_test.js?raw";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { tempDir, join } from "@tauri-apps/api/path";
import { exists, mkdir, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { ChildProcess, Command } from "@tauri-apps/plugin-shell";
import { openUrl } from "@tauri-apps/plugin-opener";
initMongoQueryGlobals();

enum TsOrJs {
  js = "javascript",
  ts = "typescript",
}

class CompilerOutput {
  constructor(
    public out: ChildProcess<string>,
    public compileTime: number,
    public jsCode?: string,
  ) {}

  errMsg(): string {
    return this.out.stderr ? `${this.out.stdout}\n${this.out.stderr}` : this.out.stdout;
  }
}
function dropLastLine(str) {
  return str.split(/\r?\n/).slice(0, -1).join("\n");
}

class SaveTsOption {
  static key = "dbpal-ts-options";

  static getStr() {
    try {
      const op = localStorage.getItem(SaveTsOption.key);
      if (op) {
        return op;
      }
    } catch (e) {}
    return `
{
    "compilerOptions": {
        "target": "esnext",
        "lib": [
            "esnext",
            "dom"
        ]
    }
}
    `;
  }
  static get() {
    const ts = SaveTsOption.getStr();
    return JSON.parse(ts);
  }
  static set(x: object) {
    localStorage.setItem(SaveTsOption.key, JSON.stringify(x, null));
  }
}

class TSCompiler {
  static async compile(value: string, sourceMap: boolean): Promise<CompilerOutput> {
    // dir

    const data_dir = await tempDir();
    const rootPath = await join(data_dir, "dbpal-mongo-query-ts-compile");
    if (!(await exists(rootPath))) {
      await mkdir(rootPath);
    }
    console.info("root path is:", rootPath);
    // write main.ts
    const mainTsFilePath = await join(rootPath, "main.ts");
    await writeTextFile(mainTsFilePath, value, { create: true, append: false });

    // write config file
    const optionFilePath = await join(rootPath, "tsconfig.json");
    let op = JSON.parse(SaveTsOption.getStr());
    // make sure compilerOptions.outDir
    let compilerOptions = "compilerOptions";
    if (!(compilerOptions in op)) {
      op[compilerOptions] = {};
    }

    op[compilerOptions]["outDir"] = "./dist";
    op[compilerOptions]["sourceMap"] = sourceMap;
    op[compilerOptions]["typeRoots"] = ["./types"];
    await writeTextFile(optionFilePath, JSON.stringify(op, null, 2), {
      create: true,
      append: false,
    });
    let typesPath = await join(rootPath, "types");

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
    let cmd = Command.create(TSCompiler.getCompilerCmd(), [], {
      cwd: rootPath,
    });
    const out = await cmd.execute();
    //@ts-ignore
    let compileTime = new Date() - start;
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
    let cmd = Command.create(TSCompiler.getCompilerCmd(), ["--version"]);
    const out = await cmd.execute();
    return out.stdout;
  }

  static getCompilerCmd() {
    return isWindows() ? "run-tsc-cmd" : "run-tsc";
  }
}

function TsOption(props: { handleClose: () => void }) {
  const { handleClose } = props;

  const [jsonErr, setJsonErr] = useState<ErrAndLoc | null>(null);
  const setting = useContext(SettingContext);
  const [value, setValue] = useState(
    JSON.stringify(SaveTsOption.get(), null, setting.editor.tabSize),
  );
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
        handleValue={value => {
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
      ></SimpleFlexEditor>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <Button onClick={handleSave}> save </Button>
        <Link
          href="#"
          underline="hover"
          onClick={e => {
            e.preventDefault();
            openUrl("https://www.typescriptlang.org/tsconfig/");
          }}
        >
          options
        </Link>
      </Stack>
    </Stack>
  );
}

function TypeScript(props: {
  ts: string;
  setTS;
  editor: editor.IStandaloneCodeEditor;
  compileTime?: number;
}) {
  const { ts, setTS, editor, compileTime } = props;
  const [showEditOption, setShowEditOption] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [tscVersion, setTscVersion] = useState("");

  useEffect(() => {
    if (ts == TsOrJs.js) {
      return;
    }

    TSCompiler.getTscVersion()
      .then(v => {
        setTscVersion(v);
      })
      .catch(e => {
        showNotification("error", "is typescript  global installed?", e);
      });
  }, [ts]);
  return (
    <>
      <Autocomplete
        options={[TsOrJs.js, TsOrJs.ts]}
        value={ts}
        style={{ width: "12rem" }}
        renderInput={params => <TextField {...params} label={"language"} />}
        freeSolo={false}
        onChange={(_, name) => {
          (async () => {
            if (name == TsOrJs.js) {
              const confirm = await secondConfirm({
                title: "Warning",
                content: "Switching to javascript may lose some type definitions",
                kind: "warning",
              });
              if (!confirm) {
                return;
              }
              let result = await TSCompiler.compile(editor.getValue(), false);
              if (result.out.code != 0) {
                showNotification(
                  "warning",
                  "typescript compile error,can safe change to javascript",
                  result.errMsg(),
                );
                return;
              }
              editor.setValue(result.jsCode);
            }
            setTS(name);
          })();
        }}
      />
      {ts == TsOrJs.ts && tscVersion && (
        <Tooltip title="typescript version">
          <Chip color="success" label={tscVersion}></Chip>
        </Tooltip>
      )}
      {ts == TsOrJs.ts && (
        <Tooltip title="click this button to modify tsconfig.json">
          <Button
            variant="outlined"
            onClick={() => {
              setShowEditOption(true);
            }}
          >
            tsconfig?
          </Button>
        </Tooltip>
      )}
      {ts == TsOrJs.ts && compileTime && (
        <Tooltip title="compile time">
          <Typography> {compileTime}ms </Typography>
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
              title="edit tsconfig.json"
              handleClose={() => {
                setShowEditOption(false);
              }}
              noTextTransform
            ></ModalHeader>
            <TsOption
              handleClose={() => {
                setShowEditOption(false);
              }}
            ></TsOption>
          </Box>
        </Modal>
      )}
    </>
  );
}

let lastLanguage = TsOrJs.js;
function MongoQuery(props: {
  uuid: string;
  favoriteName?: string;
  saveFavoriteSql: (name: string, sql: string) => void;
  value?: string;
}) {
  const { uuid, favoriteName: oldFavoriteName, saveFavoriteSql, value } = props;
  const [showSetFavoriteName, setShowFavoriteName] = useState(false);
  const [favoriteName, setFavoriteName] = useState(oldFavoriteName);
  const [showQueryLog, setShowQueryLog] = useState(false);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [pos, setPos] = useState<Position>(null);
  const [executeTimes, setExecuteTimes] = useState(0);
  const setting = useContext(SettingContext);
  const [db] = useState(new MongoDBQuery(uuid));
  const [cleanFns] = useState([]);
  const { showNotification } = useContext(NotificationContext);
  const [bson, setBson] = useState(null);
  const [ts, setTS] = useState<TsOrJs>(lastLanguage);
  const [compileTime, setCompileTime] = useState<number | null>(null);

  useEffect(() => {
    lastLanguage = ts;
  }, [ts]);

  const jsCodeMount = (ed: editor.IStandaloneCodeEditor, _monaco: Monaco) => {
    setEditor(ed);
    ed.focus();
    ed.onDidChangeCursorPosition(() => {
      setPos(ed.getPosition());
    });
  };
  useEffect(() => {
    import("bson").then(e => {
      setBson(e);
    });
  }, []);

  const handleExecute = async () => {
    let code = editor.getValue();
    if (!code.trim()) {
      showNotification("warning", "can't execute empty statement");
      return;
    }
    if (SaveQueryLogConfigure.get()) {
      new QueryLogHandler(uuid).save(code, true, showNotification);
    }
    if (ts == TsOrJs.ts) {
      const out = await TSCompiler.compile(code, true);
      setCompileTime(out.compileTime);
      if (out.out.code != 0) {
        showNotification("warning", "compile failed", out.errMsg());
        return;
      }
      code = out.jsCode;
    }
    setExecuteTimes(e => e + 1);
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
    <Stack style={{ position: "relative", height: "100%" }}>
      <QueryLog
        showQueryLog={showQueryLog}
        setShowQueryLog={setShowQueryLog}
        uuid={uuid}
        kind={DataSourceKind.Mongo}
        executeTimes={executeTimes}
      ></QueryLog>
      <Stack direction="row" sx={{ alignItems: "center" }}>
        <TypeScript
          ts={ts}
          setTS={setTS}
          editor={editor}
          compileTime={compileTime}
        ></TypeScript>
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
        <Tooltip
          title={
            !favoriteName
              ? "save sql to favorite."
              : `save favorite sql as named '${favoriteName}'`
          }
        >
          <IconButton
            onClick={() => {
              const value = editor.getValue();
              if (!value) {
                showNotification("warning", "can't save empty statement.");
                return;
              }
              if (favoriteName) {
                saveFavoriteSql(favoriteName, value);
              } else {
                setShowFavoriteName(true);
              }
            }}
            color="primary"
          >
            <FaBookmark style={{ fontSize: "1.3rem" }} />
          </IconButton>
        </Tooltip>
        <LoadStoreFile
          getQuery={() => {
            return editor.getValue();
          }}
          kind={DataSourceKind.Mongo}
          handleLoadFile={(path, text) => {
            if (path.endsWith(".ts")) {
              setTS(TsOrJs.ts);
            }
            editor.setValue(text);
          }}
        ></LoadStoreFile>

        <Tooltip title="query log">
          <IconButton
            onClick={() => {
              setShowQueryLog(prev => !prev);
            }}
            color="primary"
          >
            <EventNoteIcon></EventNoteIcon>
          </IconButton>
        </Tooltip>

        {cleanFns.length > 0 && (
          <Button
            onClick={() => {
              let has_err = false;
              for (const callback of cleanFns) {
                try {
                  callback();
                } catch (e) {
                  has_err = true;
                  showNotification("error", "clean error", e);
                }
              }
              cleanFns.length = 0; // clean function
              if (!has_err) {
                showNotification("success", "call those function finished");
              }
            }}
            variant="outlined"
          >
            clear resource
          </Button>
        )}

        <Tooltip
          title=<CleanFunctionDoc></CleanFunctionDoc>
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
            <Typography variant="h6">
              open dev tools:
              {isMacOs() ? "Command +Option + i" : "Ctrl + Shift + i"}
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
      </Stack>
      <div
        style={{
          height: "100px",
          flex: 1,
        }}
      >
        <Editor
          width="99%"
          height="99%"
          onMount={jsCodeMount}
          language={ts}
          options={options}
          value={value}
          theme={setting.editorTheme()}
        ></Editor>
        <EditorBar pos={pos}></EditorBar>
      </div>
      {showSetFavoriteName && (
        <Modal
          open={showSetFavoriteName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFavoriteName(false);
          })}
        >
          <Box sx={modalStyle} width="45vw">
            <ModalHeader
              title="Set Favorite Name"
              handleClose={() => {
                setShowFavoriteName(false);
              }}
            ></ModalHeader>
            <Stack>
              <TextField
                fullWidth
                label="name"
                value={favoriteName}
                onChange={e => {
                  setFavoriteName(e.target.value);
                }}
                required
              ></TextField>
              <Stack direction="row">
                <IconButton
                  color="primary"
                  onClick={() => {
                    if (!favoriteName) {
                      showNotification("warning", "name can't be empty");
                      return;
                    }
                    saveFavoriteSql(favoriteName, editor.getValue());
                    setShowFavoriteName(false);
                  }}
                >
                  <FaBookmark> </FaBookmark>
                </IconButton>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
      {/* // we must use db etc ...  */}
      {db && <span></span>}
      {cleanFns && <span></span>}
      {bson && <span></span>}
    </Stack>
  );
}

function CleanFunctionDoc() {
  return (
    <Stack>
      <SyntaxHighlighter
        style={syntaxHightLightTheme()} // Apply the chosen theme
        language={"javascript"}
        PreTag="div"
      >
        {intervalCleanTest}
      </SyntaxHighlighter>
    </Stack>
  );
}

export { MongoQuery };
