import React from 'react';
import ReactDOM from 'react-dom/client';
import './styles.css';
import './comp/MainPage';
import MainPage from './comp/MainPage';
import { BSON, EJSON, ObjectId } from 'bson';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <MainPage></MainPage>
  </React.StrictMode>
);


(async () => {
  new BSON.BSONError("123123");
})();
