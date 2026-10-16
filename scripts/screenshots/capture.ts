import { mkdir } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
import { createServer } from "vite";

const outDir = process.argv[2];
if (!outDir) {
  console.error("Usage: node scripts/screenshots/capture.ts <output-dir>");
  process.exit(1);
}

const PAGES = ["General", "Hotkey", "Words", "History", "Mode", "Permissions"];
const SCHEMES = ["dark", "light"] as const;

await mkdir(outDir, { recursive: true });

const server = await createServer({ server: { strictPort: false } });
await server.listen();
const browser = await chromium.launch();

try {
  const url = new URL("/scripts/screenshots/index.html", server.resolvedUrls?.local[0]);

  for (const colorScheme of SCHEMES) {
    const page = await browser.newPage({ colorScheme, deviceScaleFactor: 2 });
    await page.goto(url.href);
    await page.getByText("Ready", { exact: true }).waitFor();

    for (const name of PAGES) {
      await page.getByRole("button", { name, exact: true }).click();
      await page.getByRole("heading", { name, exact: true }).waitFor();

      const file = path.join(outDir, `screenshot-${name.toLowerCase()}-${colorScheme}.png`);
      await page
        .locator("#shot")
        .screenshot({ path: file, omitBackground: true, animations: "disabled" });
      console.log(file);
    }

    await page.close();
  }
} finally {
  await browser.close();
  await server.close();
}
