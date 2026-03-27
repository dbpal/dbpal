import React, { useRef, useEffect, useState } from "react";
import * as monaco from "monaco-editor";

interface MonacoEditorProps {
  value: string;
  language: string;
  options?: monaco.editor.IStandaloneEditorConstructionOptions;
  onChange?: (value: string) => void;
  theme?: "vs-dark" | "light" | string;
  height?: string;
  width: string;
}

// eslint-disable-next-line react/function-component-definition
const MonacoEditor: React.FC<MonacoEditorProps> = ({
  value,
  language,
  options,
  onChange,
  theme = "vs-dark",
  height = "500px",
  width = "100%",
}) => {
  const [editorRef, setEditor] = useState<HTMLDivElement>(null);
  const monacoInstance = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);

  useEffect(() => {
    if (!editorRef) {
      return;
    }
    console.log("create editor ");
    if (!monacoInstance.current) {
      // 1. 初始化编辑器
      monacoInstance.current = monaco.editor.create(editorRef.current, {
        value,
        language,
        theme,
        automaticLayout: true, // 自动响应容器大小
        ...options,
      });

      // 2. 监听内容变化
      monacoInstance.current.onDidChangeModelContent(() => {
        const newValue = monacoInstance.current?.getValue();
        if (onChange && newValue !== undefined) {
          onChange(newValue);
        }
      });
    }

    // 3. 组件卸载时销毁实例，防止内存泄漏
    return () => {
      monacoInstance.current?.dispose();
    };
  }, [editorRef, language, onChange, options, theme, value]);

  // 响应外部 value 变化 (受控组件逻辑)
  useEffect(() => {
    if (monacoInstance.current && value !== monacoInstance.current.getValue()) {
      monacoInstance.current.setValue(value);
    }
  }, [value]);

  // 响应语言变化
  useEffect(() => {
    if (monacoInstance.current) {
      const model = monacoInstance.current.getModel();
      if (model) {
        monaco.editor.setModelLanguage(model, language);
      }
    }
  }, [language]);

  return <div ref={setEditor} style={{ height, width }} />;
};

export { MonacoEditor };
