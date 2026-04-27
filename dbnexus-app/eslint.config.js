import js from "@eslint/js";
import reactPlugin from "eslint-plugin-react";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  reactPlugin.configs.flat.recommended, // 自带 jsx-key: 'error'
  {
    ignores: ["src/comp/MongoVar.ts"],
  },
  {
    files: ["**/*.{ts,tsx}"],

    plugins: {
      react: reactPlugin,
      "react-hooks": reactHooks,
    },
    languageOptions: {
      parserOptions: {
        project: "./tsconfig.json", // 启用类型感知（可选）
      },
    },
    rules: {
      "prefer-const": "off",
      "@typescript-eslint/ban-ts-comment": "off",
      "@typescript-eslint/no-explicit-any": "off",
      /* ====== 核心 Hooks 规则 ====== */
      "react-hooks/rules-of-hooks": "error", // 禁止在条件/循环里写 Hooks
      "react-hooks/exhaustive-deps": "warn", // 检查 useEffect/useCallback 等 deps 数组
      "react-hooks/immutability": "error", // 禁止直接修改 useState 返回值

      /* ====== JSX 语法 ====== */
      "react/jsx-key": "error", // 数组元素必须写 key
      "react/jsx-no-duplicate-props": "error", // 禁止同名 props
      "react/jsx-no-undef": "error", // 禁止在 JSX 里使用未定义变量
      "react/jsx-uses-react": "off", // React 17+ 可关
      "react/jsx-uses-vars": "error", // 在 JSX 用到的变量标记为“已使用”
      "react/jsx-no-target-blank": "warn", // 禁止 <a target="_blank" 无 rel="noreferrer"
      "react/jsx-no-comment-textnodes": "warn", // 防止把 {/* 注释 */} 写成 // 注释

      /* ====== 组件写法 ====== */
      "react/function-component-definition": ["warn"], // 统一函数组件写法
      "react/prefer-stateless-function": "warn", // 无状态组件建议写成函数
      "react/no-unused-state": "error", // 删除从未使用的 state
      "react/no-direct-mutation-state": "error", // 禁止 this.state.xxx = 123
      "react/no-deprecated": "warn", // 禁止使用已废弃的 API
      "react/no-children-prop": "warn", // 防止把元素当 children 字符串传
      "react/no-danger-with-children": "error", // 禁止同时用 dangerouslySetInnerHTML 和 children
      "react/no-find-dom-node": "error", // 禁止 findDOMNode（已废弃）
      "react/no-render-return-value": "error", // 禁止用 ReactDOM.render 返回值
      "react/no-string-refs": "error", // 禁止字符串 ref
      "react/no-unescaped-entities": "warn", // 防止 > } 等字符直接出现在 JSX 文本
      "react/no-unknown-property": "error", // 禁止非法 DOM 属性（如 class 而不是 className）
      "react/prop-types": "off", // 用 TS 时关掉
      "react/react-in-jsx-scope": "off", // React 17+ 无需 import React
      "react/self-closing-comp": "warn", // 空标签建议自关闭 <div />
    },
    settings: {
      react: { version: "detect" },
    },
  },
];
