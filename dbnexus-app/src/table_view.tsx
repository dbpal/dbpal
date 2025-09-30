import ReactDOM from 'react-dom/client';
import './app.css';
import React from 'react';
import { keyNotInTable } from './comp/Common';
import { windowInit, abc } from './comp/helper.ts';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <div>123 .... </div>
  </React.StrictMode>
);

console.log('this is table view ... ');
abc();
// console.log(keyNotInTable);
