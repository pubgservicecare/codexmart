import type { Config } from 'tailwindcss';

// Design tokens for CODE X MART. Colours are referenced through CSS
// variables (see src/styles/index.css) so the palette lives in one place.
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Inter Variable"', '"Segoe UI Variable Text"', '"Segoe UI"', 'system-ui', 'sans-serif'],
        mono: ['"Cascadia Mono"', 'Consolas', 'ui-monospace', 'monospace'],
      },
      colors: {
        brand: {
          50: '#EEF0FF',
          100: '#E0E3FF',
          200: '#C7CBFE',
          300: '#A4A9FC',
          400: '#8186F8',
          500: '#6366F1',
          600: '#4F46E5',
          700: '#4338CA',
          800: '#3730A3',
          900: '#2E2A7A',
        },
        ink: {
          950: '#0B0D1A',
          900: '#11142A',
          800: '#1A1E3A',
          700: '#262B4D',
          600: '#3A4066',
        },
      },
      boxShadow: {
        card: '0 1px 2px rgba(16, 24, 40, 0.04), 0 1px 3px rgba(16, 24, 40, 0.06)',
        pop: '0 12px 32px -8px rgba(16, 24, 40, 0.18), 0 4px 8px -4px rgba(16, 24, 40, 0.08)',
      },
      fontSize: {
        '2xs': ['0.6875rem', { lineHeight: '1rem' }],
      },
    },
  },
  plugins: [],
} satisfies Config;
