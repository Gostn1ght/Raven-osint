/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,ts,js}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        raven: {
          50:  "#f5f3ff",
          100: "#ede9fe",
          200: "#ddd6fe",
          300: "#c4b5fd",
          400: "#a78bfa",
          500: "#8b5cf6",
          600: "#7c3aed",
          700: "#6d28d9",
          800: "#5b21b6",
          900: "#4c1d95",
          950: "#0f0720",
        },
        surface: {
          50:  "#f8f7ff",
          900: "#0d0b14",
          950: "#07060e",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "Fira Code", "monospace"],
        sans: ["Inter", "system-ui", "sans-serif"],
      },
      animation: {
        "pulse-slow": "pulse 3s cubic-bezier(0.4,0,0.6,1) infinite",
        "scan":       "scan 2s linear infinite",
      },
      keyframes: {
        scan: {
          "0%":   { transform: "translateY(-100%)", opacity: "0.6" },
          "100%": { transform: "translateY(400%)",  opacity: "0" },
        },
      },
    },
  },
  plugins: [],
};
