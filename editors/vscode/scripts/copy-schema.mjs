// The config's JSON Schema is generated at the repository root (`cargo xtask codegen`); the
// extension ships a copy for its `yamlValidation` contribution.
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
mkdirSync(join(root, "schemas"), { recursive: true });
copyFileSync(join(root, "../../schemas/sudsy.schema.json"), join(root, "schemas/sudsy.schema.json"));
