import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwind from 'tailwindcss';
import autoprefixer from 'autoprefixer';
import baseTheme from '../../tailwind.config';

// The owner's License Generator UI. It reuses the design system and UI
// components of the main app (alias "@") but is built and shipped as a
// separate program, never inside the customer installer.
const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  root: here('.'),
  plugins: [react()],
  resolve: {
    alias: { '@': here('../../src'), '@gen': here('./src') },
  },
  css: {
    postcss: {
      plugins: [
        tailwind({ ...baseTheme, content: [here('./index.html'), here('./src/**/*.{ts,tsx}'), here('../../src/components/**/*.{ts,tsx}')] }),
        autoprefixer(),
      ],
    },
  },
  clearScreen: false,
  server: { port: 1430, strictPort: true },
  build: { outDir: here('./dist'), emptyOutDir: true, target: 'es2021', sourcemap: false },
});
