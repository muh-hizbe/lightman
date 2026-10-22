// Generates favicon-16.png, favicon-32.png, and icon.ico from public/logo.png.
// Run this after replacing public/logo.png with a new image.
// Usage: node scripts/generate-logo.mjs

import sharp from "sharp";
import pngToIco from "png-to-ico";
import { mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";

const SOURCE = "public/logo.png";

if (!existsSync(SOURCE)) {
  console.error(`ERROR: ${SOURCE} not found. Place your logo at that path first.`);
  process.exit(1);
}

async function main() {
  await mkdir("public", { recursive: true });
  await mkdir("src-tauri/icons", { recursive: true });

  // Favicons for the web app (served at /favicon-*.png)
  for (const size of [16, 32]) {
    const dest = `public/favicon-${size}.png`;
    await sharp(SOURCE).resize(size, size).png().toFile(dest);
    console.log("✓", dest);
  }

  // icon.ico — multi-size ICO for Windows title bar / taskbar / installer
  const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256];
  const icoBuffers = await Promise.all(
    ICO_SIZES.map((s) => sharp(SOURCE).resize(s, s).png().toBuffer())
  );
  const ico = await pngToIco(icoBuffers);
  const icoDest = "src-tauri/icons/icon.ico";
  await writeFile(icoDest, ico);
  console.log("✓", icoDest, `(${(ico.length / 1024).toFixed(0)} KB, ${ICO_SIZES.join("/")}px)`);

  console.log(
    "\nDone! Also run `npx tauri icon public/logo.png` to regenerate all other Tauri icon formats."
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
