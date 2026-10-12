import { ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useUserSettings } from "./store";
import { Paper, Stack, Typography } from "@mui/material";
import { editor, MarkerSeverity } from "monaco-editor";

interface MonacoEditorProps {
  value?: string;
  language: string;
  onMount: (_: editor.IStandaloneCodeEditor) => void;
  readOnly?: boolean;
  barLeft?: ReactNode;
}

// Do not set textAlign arbitrarily; it is an inherited property and affects how the Monaco editor is displayed.

function MonacoEditor({
  value,
  language,
  onMount,
  readOnly,
  barLeft,
}: MonacoEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);

  const [editorIns, setIns] = useState<editor.IStandaloneCodeEditor>(null);

  const [pos, setPos] = useState(null);
  const settting = useUserSettings();

  const [markers, setMarkers] = useState<editor.IMarker[]>([]);

  const options: editor.IEditorOptions & editor.IGlobalEditorOptions = useMemo(
    () => ({
      fontSize: settting.appearance.fontSize,
      fontWeight: settting.appearance.fontWeight,
      fontFamily: settting.appearance.fontFamily,
      readOnly: readOnly,
      ...JSON.parse(settting.editor.editorOption),
    }),
    [
      readOnly,
      settting.appearance.fontFamily,
      settting.appearance.fontSize,
      settting.appearance.fontWeight,
      settting.editor.editorOption,
    ],
  );

  useEffect(() => {
    if (!editorIns) {
      // 1. Initialize the editor.
      const ed = editor.create(editorRef.current, { automaticLayout: true });
      setIns(ed);
      ed.focus();
    }
    // 3. Destroy the instance when the component unmounts to prevent memory leaks.
    return () => {
      editorIns?.dispose();
    };
  }, [editorIns]);

  useEffect(() => {
    if (!editorIns) {
      return;
    }
    onMount(editorIns);
  }, [editorIns, onMount]);

  useEffect(() => {
    if (!editorIns) {
      return;
    }
    if (typeof value == "string" && value != editorIns.getValue()) {
      editorIns.setValue(value);
    }
  }, [editorIns, value]);

  useEffect(() => {
    if (!editorIns) {
      return;
    }
    editorIns.updateOptions(options);
  }, [editorIns, options]);
  const instanceModel = editorIns ? editorIns.getModel() : undefined;

  useEffect(() => {
    if (!editorIns) {
      return;
    }
    if (!language) {
      return;
    }
    if (!instanceModel) {
      return;
    }
    editor.setModelLanguage(instanceModel, language);
  }, [editorIns, instanceModel, language]);
  useEffect(() => {
    if (!editorIns) {
      return;
    }
    let d = editorIns.onDidChangeCursorPosition(() => {
      setPos(editorIns.getPosition());
    });
    return () => {
      d.dispose();
    };
  }, [editorIns]);
  useEffect(() => {
    if (!editorIns) return;

    // Listen for changes to decorations (such as squiggles), which usually means validation has completed.
    const d = editorIns.onDidChangeModelDecorations(() => {
      const model = editorIns.getModel();
      if (!model) return;

      const markers = editor.getModelMarkers({
        resource: model.uri,
      });

      setMarkers(markers);
    });

    return () => d.dispose();
  }, [editorIns]);

  return (
    <div
      style={{
        height: "99%",
        width: "99%",
        overflow: "auto",
        display: "flex",
        flexDirection: "column",
        position: "relative",
      }}>
      <div
        ref={editorRef}
        style={{
          height: "100px",
          flexGrow: 1,
          flexBasis: 0,
        }}
      />

      {markers.length > 0 && (
        <Paper
          style={{
            position: "absolute",
            zIndex: 14,
            left: 0,
            bottom: 0,
            width: "60%",
            overflow: "auto",
          }}>
          {markers.map((marker, index) => {
            return (
              <Stack
                direction="row"
                key={index}
                sx={{
                  alignItems: "center",
                  cursor: "pointer",
                }}
                onClick={() => {
                  editorIns.setPosition({
                    lineNumber: marker.startLineNumber,
                    column: marker.startColumn,
                  });
                  editorIns.focus();
                }}>
                <Typography
                  color={(function () {
                    switch (marker.severity) {
                      case MarkerSeverity.Error:
                        return "error";
                      case MarkerSeverity.Warning:
                        return "warning";
                      case MarkerSeverity.Info:
                        return "info";
                      default:
                        return "";
                    }
                  })()}>
                  {marker.startLineNumber}:{marker.startColumn}
                  &nbsp;&nbsp;
                  {marker.message}
                </Typography>
              </Stack>
            );
          })}
        </Paper>
      )}

      <Stack
        style={{ justifyContent: "space-between", alignItems: "center" }}
        direction="row">
        {barLeft ? barLeft : <span />}

        <Stack direction="row" style={{ paddingLeft: "2rem", paddingRight: "3rem" }}>
          <span>Ln {pos ? pos.lineNumber : ""}</span>
          <span>Col {pos ? pos.column : ""}</span>
        </Stack>
      </Stack>
    </div>
  );
}

export { MonacoEditor };
