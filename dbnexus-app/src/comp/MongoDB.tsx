import Editor from '@monaco-editor/react';
import { Stack } from '@mui/material';

import { loader } from '@monaco-editor/react';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import { formatMongoBson } from './MongoBsonCompiler';

const mongoBsonKeywords = /\b(as|int|long|date|timestamp|decimal|binary)\b/;
function mongoBsonAutoCompletion(): languages.CompletionList {
  return {
    suggestions: [
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
        let result = formatMongoBson(text, options.tabSize);

        if (typeof result === 'string') {
          return [{ range: model.getFullModelRange(), text: result }];
        } else {
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

  return (
    <Stack style={{ height: '200px' }}>
      <Editor language='mongoBson' value={content}></Editor>
    </Stack>
  );
}

export { MongoBson };
