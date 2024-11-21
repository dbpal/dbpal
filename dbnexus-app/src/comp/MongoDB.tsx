import Editor from '@monaco-editor/react';
import { Stack } from '@mui/material';

import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import {
  formatMongoBsonStr,
  MongoBsonBinaryAllSubType,
  MongoBsonError,
} from './MongoBsonCompiler';
import { useState } from 'react';

const mongoBsonKeywords =
  /\b(as|int|long|date|timestamp|decimal|binary|generic|function|byte_array|uuid|md5|encrypted|column|sensitive|vector|user_define|true|false|null|undefined|code)\b/;
function mongoBsonAutoCompletion(): languages.CompletionList {
  let generateSubTypes = MongoBsonBinaryAllSubType.allValues().map((e) => {
    return {
      label: e,
      insertText: e,
      kind: languages.CompletionItemKind.Operator,
    } as languages.CompletionItem;
  });

  return {
    suggestions: [
      ...generateSubTypes,
      {
        label: 'as',
        kind: languages.CompletionItemKind.Keyword,
        insertText: 'as',
        range: undefined,
      },
      {
        label: 'int',
        kind: languages.CompletionItemKind.Class,
        insertText: 'int',
        range: undefined,
      },
      {
        label: 'long',
        kind: languages.CompletionItemKind.Class,
        insertText: 'long',
        range: undefined,
      },
      {
        label: 'date',
        kind: languages.CompletionItemKind.Class,
        insertText: 'date',
        range: undefined,
      },
      {
        label: 'timestamp',
        kind: languages.CompletionItemKind.Class,
        insertText: 'timestamp',
        range: undefined,
      },
      {
        label: 'decimal',
        kind: languages.CompletionItemKind.Class,
        insertText: 'decimal',
        range: undefined,
      },
      {
        label: 'binary',
        kind: languages.CompletionItemKind.Class,
        insertText: 'binary',
        range: undefined,
      },
      {
        label: 'ObjectID',
        kind: languages.CompletionItemKind.Class,
        insertText: 'ObjectID',
        range: undefined,
      },
      {
        label: 'code',
        kind: languages.CompletionItemKind.Class,
        insertText: 'code',
        range: undefined,
      },
      {
        label: 'true',
        kind: languages.CompletionItemKind.Constant,
        insertText: 'true',
        range: undefined,
      },
      {
        label: 'false',
        kind: languages.CompletionItemKind.Constant,
        insertText: 'false',
        range: undefined,
      },
    ],
  };
}

function initMongoBsonLanguage() {
  loader.init().then((monaco) => {
    monaco.languages.register({ id: 'mongoBson' });
    monaco.languages.setMonarchTokensProvider('mongoBson', {
      ignoreCase: true,
      tokenizer: {
        root: [[mongoBsonKeywords, 'keyword']],
      },
    });
    monaco.languages.registerDocumentFormattingEditProvider('mongoBson', {
      provideDocumentFormattingEdits(model, options, _token) {
        let text = model.getValue();
        let result = formatMongoBsonStr(text, options.tabSize);
        if (typeof result === 'string') {
          return [{ range: model.getFullModelRange(), text: result }];
        } else {
          let e = result as MongoBsonError;
          let msg = `${e.loc.start.line}:${e.loc.start.column}  ${e.msg}`;
          console.log(msg);
          alert(msg);
          throw result;
        }
      },
    });
    monaco.languages.registerCompletionItemProvider('mongoBson', {
      provideCompletionItems: (
        _model: editor.ITextModel,
        _position: Position,
        _context: any,
        _token: CancellationToken
      ) => {
        return mongoBsonAutoCompletion();
      },
    });
  });
}

initMongoBsonLanguage();

function MongoBson(props: { content: string }) {
  let { content } = props;
  const [text, setText] = useState('');
  const [heigth, setHeigth] = useState('200px');

  const handleEditorDidMount = (ce: editor.IStandaloneCodeEditor) => {
    ce.onDidChangeModelContent((_event) => {
      setText(ce.getValue());
    });
    ce.onDidChangeModelContent(() => {
      const lineCount = ce.getModel().getLineCount();
      const lineHeight = ce.getOption(editor.EditorOption.lineHeight);
      const padding = 20; // 添加一些额外的填充
      // 计算新的高度
      let newHeight = lineCount * lineHeight + padding;
      if (newHeight > 300) {
        newHeight = 300;
      }
      setHeigth(`${newHeight}px`);
    });
  };

  return (
    <Stack style={{ height: heigth }}>
      <Editor
        onMount={handleEditorDidMount}
        language='mongoBson'
        value={content}
      ></Editor>
    </Stack>
  );
}

export { MongoBson };
