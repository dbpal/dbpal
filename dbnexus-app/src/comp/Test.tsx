import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));

import React, { createContext, useContext } from 'react';
import { NameAndTypeContext } from './Common';

const App = () => {
  return (
    <NameAndTypeContext.Provider value={{ uuid: '1', conn_type: 'xxxx' }}>
      <ThemeComponent />
      <NameAndTypeContext.Provider value={{ uuid: '1' }}>
        <ThemeComponent />
      </NameAndTypeContext.Provider>
    </NameAndTypeContext.Provider>
  );
};

const ThemeComponent = () => {
  const { conn_type } = useContext(NameAndTypeContext);
  return (
    <div>
      <h2> {conn_type ? conn_type : 'empty'} </h2>
    </div>
  );
};

export default App;

root.render(
  <React.StrictMode>
    <App></App>
  </React.StrictMode>
);
