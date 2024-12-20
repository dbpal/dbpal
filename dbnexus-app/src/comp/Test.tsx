import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));

import React, { createContext, useContext } from 'react';

// 创建一个Context
const ThemeContext = createContext();

const App = () => {
  return (
    <ThemeContext.Provider value='light'>
      <ThemeComponent />
    </ThemeContext.Provider>
  );
};

const ThemeComponent = () => {
  return (
    <div>
      <h2>Outer Theme</h2>
      <ThemeContext.Provider value='dark'>
        <InnerComponent />
      </ThemeContext.Provider>
    </div>
  );
};

const InnerComponent = () => {
  const theme = useContext(ThemeContext);

  return (
    <div
      style={{
        background: theme === 'dark' ? '#333' : '#FFF',
        color: theme === 'dark' ? '#FFF' : '#000',
      }}
    >
      <h3>Inner Component Theme: {theme}</h3>
    </div>
  );
};

export default App;

root.render(
  <React.StrictMode>
    <App></App>
  </React.StrictMode>
);
