# dbnexus




## how to run dbnexus-app 
~~~
cd dbnexus-app
pnpm run dev
cargo r
~~~



## 本地加载editor的依赖
~~~
pnpm install monaco-editor
mkdir public/monaco-editor

import { loader } from '@monaco-editor/react';

loader.config({
  paths: { vs: '/monaco-editor/min/vs' },
});

~~~