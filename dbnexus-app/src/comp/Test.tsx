import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));

import React, { createContext, useContext } from 'react';
import { UuidContext } from './Common';

const App = () => {
  return (
    <UuidContext.Provider value={{ uuid: '1', conn_type: 'xxxx' }}>
      <ThemeComponent />
      <UuidContext.Provider value={{ uuid: '1' }}>
        <ThemeComponent />
      </UuidContext.Provider>
    </UuidContext.Provider>
  );
};

const ThemeComponent = () => {
  const { conn_type } = useContext(UuidContext);
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
