/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        ravens: {
          bg: "#0a0b0f",
          panel: "#111318",
          border: "#1e2130",
          accent: "#c0392b",
          accent2: "#e74c3c",
          text: "#e0e0e0",
          muted: "#6b7280",
          found: "#22c55e",
          info: "#3b82f6",
          warn: "#f59e0b",
          error: "#ef4444",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "Fira Code", "monospace"],
        sans: ["Inter", "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [],
};
