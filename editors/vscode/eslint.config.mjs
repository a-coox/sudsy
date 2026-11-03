import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist/**", "out/**", "server/**", "schemas/**", "node_modules/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  { rules: { eqeqeq: "error" } },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: { globals: { process: "readonly", console: "readonly" } },
  },
);
