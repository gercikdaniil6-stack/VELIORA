import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource-variable/manrope';
import './styles/app.css';
import './styles/features.css';
import './styles/motion.css';
import './styles/appearance.css';
import { App } from './App';
import { ToastProvider } from './components/Toast';
import { CoreProvider } from './state/core';
import { StoreProvider } from './state/store';
import { AppearanceProvider } from './theme/AppearanceProvider';
import { apply, loadLocal } from './theme/engine';

// Paint the saved theme before React mounts — no flash of the wrong theme.
apply(loadLocal());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ToastProvider>
      <CoreProvider>
        <AppearanceProvider>
          <StoreProvider>
            <App />
          </StoreProvider>
        </AppearanceProvider>
      </CoreProvider>
    </ToastProvider>
  </StrictMode>,
);
