import ReactDOM from 'react-dom/client';

import './comp/MainPage';
import MainPage from './comp/MainPage';
import { debug } from './comp/debug';
import { invoke } from '@tauri-apps/api/core';

if (!debug) {
  document.addEventListener('contextmenu', function (event) {
    event.preventDefault();
  });
}
// call every time maybe use refresh this app some what.
window.addEventListener('load', (_event) => {
  invoke('page_init_callback');
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <MainPage></MainPage>
);
