/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        // ローコントラストなミニマルパレット（グレースケール階層）
        surface: {
          DEFAULT: "#0d0d0d",     // ベース（最暗）
          1: "#131313",           // レベル1（少し明るい）
          2: "#191919",           // レベル2（もう少し明るい）
          3: "#1f1f1f",           // レベル3（パネル）
          border: {
            DEFAULT: "#181818",   // surface-1 より少し明るい（+5）
            2: "#1e1e1e",         // surface-2 より少し明るい（+5）
            3: "#242424",         // surface-3 より少し明るい（+5）
          },
        },
        text: {
          primary: "#989898",     // メインテキスト（落ち着いたグレー）
          secondary: "#707070",   // セカンダリテキスト
          muted: "#505050",       // 控えめテキスト
        },
        accent: {
          DEFAULT: "#b0b0b0",     // アクセント（ソフトな白）
          subtle: "#404040",      // 控えめアクセント
        },
      },
      fontFamily: {
        mono: [
          "JetBrains Mono",
          "Fira Code",
          "SF Mono",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },
      borderRadius: {
        none: "0",
        sm: "1px",
        DEFAULT: "2px",
        md: "3px",
      },
      fontSize: {
        xs: ["10px", { lineHeight: "14px", letterSpacing: "0.02em" }],
        sm: ["11px", { lineHeight: "16px", letterSpacing: "0.01em" }],
        base: ["12px", { lineHeight: "18px", letterSpacing: "0" }],
        lg: ["14px", { lineHeight: "20px", letterSpacing: "0" }],
      },
    },
  },
  plugins: [],
};
