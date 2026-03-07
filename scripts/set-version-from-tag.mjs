import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

const input = process.argv[2];

if (!input) {
  console.error("Usage: node scripts/set-version-from-tag.mjs <tag-or-version>");
  process.exit(1);
}

const version = input.startsWith("v") ? input.slice(1) : input;
const semverPattern =
  /^\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

if (!semverPattern.test(version)) {
  console.error(`Invalid semver version: ${input}`);
  process.exit(1);
}

async function updateJsonVersion(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  const content = await readFile(filePath, "utf8");
  const parsed = JSON.parse(content);
  parsed.version = version;
  await writeFile(filePath, `${JSON.stringify(parsed, null, 2)}\n`);
}

async function updateCargoVersion(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  const content = await readFile(filePath, "utf8");
  const updated = content.replace(
    /^version = ".*"$/m,
    `version = "${version}"`,
  );

  if (updated === content) {
    throw new Error(`Could not update version in ${relativePath}`);
  }

  await writeFile(filePath, updated);
}

await Promise.all([
  updateJsonVersion("package.json"),
  updateJsonVersion(path.join("src-tauri", "tauri.conf.json")),
  updateCargoVersion(path.join("src-tauri", "Cargo.toml")),
]);

console.log(`Updated app version to ${version}`);
