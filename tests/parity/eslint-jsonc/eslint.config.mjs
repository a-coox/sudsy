// The ESLint side of the comparison with sudsy.yaml. Each fixture group enables one rule with the
// options its file name says; the same options are written in sudsy.yaml (docs/development/parity.md).
import jsonc from "eslint-plugin-jsonc";

const only = (files, rules) => ({ files, rules });
const sortArrays = (type) => ["error", { pathPattern: "^.*$", order: { type } }];

export default [
  ...jsonc.configs["flat/base"],
  only(["dupe-keys.json"], { "jsonc/no-dupe-keys": "error" }),
  only(["sort-keys.json"], { "jsonc/sort-keys": "error" }),
  only(["sort-keys-desc.json"], { "jsonc/sort-keys": ["error", "desc"] }),
  only(["sort-keys-insensitive.json"], {
    "jsonc/sort-keys": ["error", "asc", { caseSensitive: false }],
  }),
  only(["sort-keys-natural.json"], { "jsonc/sort-keys": ["error", "asc", { natural: true }] }),
  only(["sort-keys-min3.json"], { "jsonc/sort-keys": ["error", "asc", { minKeys: 3 }] }),
  only(["sort-keys-groups.json"], {
    "jsonc/sort-keys": ["error", "asc", { allowLineSeparatedGroups: true }],
  }),
  only(["key-name-casing.json"], { "jsonc/key-name-casing": "error" }),
  only(["key-name-casing-snake.json"], {
    "jsonc/key-name-casing": ["error", { camelCase: false, snake_case: true, "kebab-case": true }],
  }),
  only(["key-name-casing-ignores.json"], {
    "jsonc/key-name-casing": ["error", { ignores: ["^\\$", "^Other$"] }],
  }),
  only(["sort-array-values.json"], { "jsonc/sort-array-values": sortArrays("asc") }),
  only(["sort-array-values-desc.json"], { "jsonc/sort-array-values": sortArrays("desc") }),
];
