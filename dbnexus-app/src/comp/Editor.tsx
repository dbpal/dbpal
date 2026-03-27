import { useEffect, useState } from "react";
import * as monaco from "monaco-editor";

interface MonacoEditorProps {
  value: string;
  language: string;
  options?: monaco.editor.IStandaloneEditorConstructionOptions;
  onChange?: (value: string) => void;
  theme?: "vs-dark" | "light" | string;
  height?: string;
  width: string;
  onMount: (_: monaco.editor.IStandaloneCodeEditor) => void;
}

function MonacoEditor({
  value,
  language,
  options,
  theme = "vs-dark",
  // height,
  // width = "100%",
  onMount,
}: MonacoEditorProps) {
  const [editorRef, setEditor] = useState<HTMLDivElement>(null);
  const [monacoInstance, setInstance] =
    useState<monaco.editor.IStandaloneCodeEditor | null>(null);

  useEffect(() => {
    if (!editorRef) {
      return;
    }

    if (!monacoInstance) {
      // 1. 初始化编辑器
      const ed = monaco.editor.create(editorRef, { automaticLayout: true });
      setInstance(ed);
    }

    // 3. 组件卸载时销毁实例，防止内存泄漏
    return () => {
      monacoInstance?.dispose();
    };
  }, [editorRef, monacoInstance]);

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
    if (!value) {
      return;
    }
    monacoInstance.setValue(value);
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

  useEffect(() => {
    if (!monacoInstance) {
      return;
    }
    monaco.editor.setModelLanguage(monacoInstance.getModel(), language);
  }, [language, monacoInstance]);
  useEffect(() => {
    monaco.editor.setTheme(theme);
  }, [theme]);

  return <div ref={setEditor} style={{ flex: 1, width: "99%", height: "99%" }} />;
}

export { MonacoEditor };
