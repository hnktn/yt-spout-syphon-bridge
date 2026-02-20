/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        // ダークテーマ用カラーパレット
        surface: {
          DEFAULT: "#1a1a1a",
          raised: "#242424",
          overlay: "#2e2e2e",
        },
        accent: {
          DEFAULT: "#e53935",  // YouTube レッド
          hover: "#f44336",
        },
      },
    },
  },
  plugins: [],
};
