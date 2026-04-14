import { useEffect, useMemo, useRef, useState } from "react";
import * as monaco from "monaco-editor";
import { useUserSettings } from "./store";

interface MonacoEditorProps {
  value?: string;
  language: string;
  onMount: (_: monaco.editor.IStandaloneCodeEditor) => void;
  readOnly?: boolean;
}

// 不能随便设置textalign 这个是继承属性 会影响monaco editor的显示呢

function MonacoEditor({ value, language, onMount, readOnly }: MonacoEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);

  const [monacoInstance, setInstance] =
    useState<monaco.editor.IStandaloneCodeEditor | null>(null);

  const settting = useUserSettings();

  const options: monaco.editor.IEditorOptions & monaco.editor.IGlobalEditorOptions =
    useMemo(
      () => ({
        fontSize: settting.appearance.fontSize,
        fontWeight: settting.appearance.fontWeight,
        fontFamily: settting.appearance.fontFamily,
        tabSize: settting.editor.tabSize,
        readOnly: readOnly,
        ...JSON.parse(settting.editor.editorOption),
      }),
      [
        readOnly,
        settting.appearance.fontFamily,
        settting.appearance.fontSize,
        settting.appearance.fontWeight,
        settting.editor.editorOption,
        settting.editor.tabSize,
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

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    if (!language) {
      return;
    }
    monaco.editor.setModelLanguage(monacoInstance.getModel(), language);
  }, [monacoInstance, language]);

  return (
    <div ref={editorRef} style={{ height: "99%", width: "99%", overflow: "auto" }} />
  );
}

export { MonacoEditor };
