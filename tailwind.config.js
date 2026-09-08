/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        forest: {
          DEFAULT: "#1B4D3E",
          50: "#E9F1EE",
          100: "#CEE0DA",
          500: "#2A6E5A",
          600: "#1B4D3E",
          700: "#143A2F",
        },
        amber: { accent: "#C97B0D" },
        cream: "#F7F7F5",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};