import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  // Relative asset URLs so the built site works from any folder or sub-path.
  base: './',
});
