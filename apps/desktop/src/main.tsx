import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import TrayPanel from './TrayPanel';
import './index.css';

// The tray dashboard is the same bundle, loaded in its own small window at "index.html#tray".
const isTray = window.location.hash === '#tray';

createRoot(document.getElementById('root')!).render(<StrictMode>{isTray ? <TrayPanel /> : <App />}</StrictMode>);
