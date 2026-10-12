import js from "@eslint/js";
import reactPlugin from "eslint-plugin-react";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  reactPlugin.configs.flat.recommended, // Includes jsx-key: 'error'
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
        project: "./tsconfig.json", // Enable type-aware rules (optional)
      },
    },
    rules: {
      "prefer-const": "off",
      "@typescript-eslint/ban-ts-comment": "off",
      "@typescript-eslint/no-explicit-any": "off",
      /* ====== Core Hooks rules ====== */
      "react-hooks/rules-of-hooks": "error", // Do not call Hooks inside conditions or loops
      "react-hooks/exhaustive-deps": "warn", // Check dependency arrays for useEffect/useCallback, etc.
      "react-hooks/immutability": "error", // Do not directly mutate values returned by useState

      /* ====== JSX syntax ====== */
      "react/jsx-key": "error", // Array elements must have a key
      "react/jsx-no-duplicate-props": "error", // Disallow duplicate prop names
      "react/jsx-no-undef": "error", // Disallow undefined variables in JSX
      "react/jsx-uses-react": "off", // Can be disabled with React 17+
      "react/jsx-uses-vars": "error", // Mark variables used in JSX as "used"
      "react/jsx-no-target-blank": "warn", // Disallow <a target="_blank"> without rel="noreferrer"
      "react/jsx-no-comment-textnodes": "warn", // Prevent writing // comments instead of {/* JSX comments */}

      /* ====== Component conventions ====== */
      "react/function-component-definition": ["warn"], // Use a consistent function component style
      "react/prefer-stateless-function": "warn", // Prefer functions for stateless components
      "react/no-unused-state": "error", // Remove state that is never used
      "react/no-direct-mutation-state": "error", // Disallow this.state.xxx = 123
      "react/no-deprecated": "warn", // Disallow deprecated APIs
      "react/no-children-prop": "warn", // Prevent passing an element as a children string
      "react/no-danger-with-children": "error", // Disallow using dangerouslySetInnerHTML and children together
      "react/no-find-dom-node": "error", // Disallow the deprecated findDOMNode
      "react/no-render-return-value": "error", // Disallow using the return value of ReactDOM.render
      "react/no-string-refs": "error", // Disallow string refs
      "react/no-unescaped-entities": "warn", // Prevent characters such as > and } from appearing directly in JSX text
      "react/no-unknown-property": "error", // Disallow invalid DOM properties (for example, class instead of className)
      "react/prop-types": "off", // Disable when using TypeScript
      "react/react-in-jsx-scope": "off", // React 17+ does not require importing React
      "react/self-closing-comp": "warn", // Prefer self-closing empty elements, such as <div />
    },
    settings: {
      react: { version: "detect" },
    },
  },
];
