// The config's JSON Schema is generated at the repository root (`cargo xtask codegen`); the
// extension ships a copy for its `yamlValidation` contribution, the repository LICENSE and the logo.
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
mkdirSync(join(root, "schemas"), { recursive: true });
copyFileSync(join(root, "../../schemas/sudsy.schema.json"), join(root, "schemas/sudsy.schema.json"));
copyFileSync(join(root, "../../docs/rules/rules.json"), join(root, "schemas/rules.json"));
copyFileSync(join(root, "../../LICENSE"), join(root, "LICENSE"));
copyFileSync(join(root, "../../assets/logo/soap_3d.png"), join(root, "icon.png"));
