import { ReactNode, useEffect, useMemo, useRef, useState } from "react";
import * as monaco from "monaco-editor";
import { useUserSettings } from "./store";
import { Paper, Stack, Typography } from "@mui/material";
import { MarkerSeverity } from "monaco-editor";

interface MonacoEditorProps {
  value?: string;
  language: string;
  onMount: (_: monaco.editor.IStandaloneCodeEditor) => void;
  readOnly?: boolean;
  barLeft?: ReactNode;
}

// 不能随便设置textalign 这个是继承属性 会影响monaco editor的显示呢

function MonacoEditor({
  value,
  language,
  onMount,
  readOnly,
  barLeft,
}: MonacoEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);

  const [monacoInstance, setInstance] =
    useState<monaco.editor.IStandaloneCodeEditor | null>(null);

  const [pos, setPos] = useState(null);
  const settting = useUserSettings();

  const [markers, setMarkers] = useState<monaco.editor.IMarker[]>([]);

  const options: monaco.editor.IEditorOptions & monaco.editor.IGlobalEditorOptions =
    useMemo(
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
    if (!monacoInstance) {
      // 1. 初始化编辑器
      const ed = monaco.editor.create(editorRef.current, { automaticLayout: true });
      setInstance(ed);
    }
    // 3. 组件卸载时销毁实例，防止内存泄漏
    return () => {
      monacoInstance?.dispose();
    };
  }, [monacoInstance]);

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    onMount(monacoInstance);
  }, [monacoInstance, onMount]);

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    if (typeof value == "string" && value != monacoInstance.getValue()) {
      monacoInstance.setValue(value);
    }
  }, [monacoInstance, value]);

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    monacoInstance.updateOptions(options);
  }, [monacoInstance, options]);
  const instanceModel = monacoInstance ? monacoInstance.getModel() : undefined;

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    if (!language) {
      return;
    }
    if (!instanceModel) {
      return;
    }
    monaco.editor.setModelLanguage(instanceModel, language);
  }, [monacoInstance, instanceModel, language]);
  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    let d = monacoInstance.onDidChangeCursorPosition(() => {
      setPos(monacoInstance.getPosition());
    });
    return () => {
      d.dispose();
    };
  }, [monacoInstance]);

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    const d = monacoInstance.onDidChangeModelContent(() => {
      const markers = monaco.editor.getModelMarkers({
        resource: monacoInstance.getModel().uri,
      });

      setMarkers(markers);
    });

    return () => {
      d.dispose();
    };
  }, [monacoInstance]);

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
          // border: "11px solid red",
        }}
      />

      {markers.length > 0 && (
        <Paper
          style={{
            position: "absolute",
            zIndex: 14,
            left: 0,
            bottom: 0,
            width: "100%",
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
                  monacoInstance.setPosition({
                    lineNumber: marker.startLineNumber,
                    column: marker.startColumn,
                  });
                  monacoInstance.focus();
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
                  {marker.startColumn}:{marker.startColumn}
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
