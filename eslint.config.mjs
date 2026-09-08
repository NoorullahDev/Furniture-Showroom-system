import { dirname } from "path";
import { fileURLToPath } from "url";
import { FlatCompat } from "@eslint/eslintrc";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const compat = new FlatCompat({ baseDirectory: __dirname });

const eslintConfig = [
  ...compat.extends("next/core-web-vitals", "next/typescript"),
  {
    ignores: [
      ".next/**",
      "out/**",
      "node_modules/**",
      "next-env.d.ts",
      "src-tauri/**",
      "next.config.js",
      "postcss.config.js",
      "tailwind.config.js",
      "eslint.config.mjs",
    ],
  },
  {
    files: ["src/lib/tauri/client.ts"],
    rules: {
      // Typed invoke layer intentionally reaches into the global `__TAURI_INTERNALS__`.
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
];

export default eslintConfig;