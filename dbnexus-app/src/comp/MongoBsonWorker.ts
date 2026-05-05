import { editor } from "monaco-editor";
import {
  mongoBsonCompilerOwner,
  MongoBsonError,
  parseMongoBson,
} from "./MongoBsonCompiler";

self.onmessage = (event: MessageEvent) => {
  const markers: editor.IMarker[] = [];
  const { code, id, resource } = event.data;

  try {
    parseMongoBson(code).compile();
  } catch (e) {
    if (e instanceof MongoBsonError) {
      markers.push({
        owner: mongoBsonCompilerOwner,
        severity: 8,
        message: e.msg,
        startLineNumber: e?.loc.start.line,
        startColumn: e?.loc.start.column + 1,
        endLineNumber: e?.loc.end.line,
        endColumn: e?.loc.end.column + 1,
        resource,
      });
    } else {
      console.error("compile should not throw other error", e);
    }
  }

  postMessage({ id, markers });
};
