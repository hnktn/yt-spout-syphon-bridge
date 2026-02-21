/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        // ミニマルパレット（コントラスト調整済み）
        surface: {
          DEFAULT: "#0d0d0d",     // ベース（最暗）
          1: "#161616",           // レベル1
          2: "#202020",           // レベル2
          3: "#2a2a2a",           // レベル3（パネル）
          border: {
            DEFAULT: "#2a2a2a",   // ベースボーダー
            2: "#333333",         // レベル2ボーダー
            3: "#3d3d3d",         // レベル3ボーダー
          },
        },
        text: {
          primary: "#d0d0d0",     // メインテキスト（明るいグレー）
          secondary: "#909090",   // セカンダリテキスト
          muted: "#606060",       // 控えめテキスト
        },
        accent: {
          DEFAULT: "#e05252",     // アクセント（赤）
          subtle: "#4a2020",      // 控えめアクセント
          green: "#4ade80",       // アクティブ状態（緑）
          "green-dim": "#22c55e", // アクティブ状態（暗い緑）
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
