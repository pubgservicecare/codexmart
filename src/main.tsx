import React from 'react';
import ReactDOM from 'react-dom/client';
import '@fontsource-variable/inter';
import './styles/index.css';
import { App } from './App';
import { setLanguage } from './lib/i18n';

setLanguage('en');

// A shop till should not show the browser's right-click menu (reload,
// inspect, ...) except where it helps: in text fields for copy/paste.
if (import.meta.env.PROD) {
  document.addEventListener('contextmenu', (e) => {
    const el = e.target as HTMLElement;
    if (!el.closest('input, textarea')) e.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
