// Versions 900 px de large des captures du site (srcset : les écrans étroits ne chargent pas les 1800 px).
//   node site/tools/shots-small.mjs   (dans site/web : sharp vient d'Astro)
import { createRequire } from 'node:module'
import fs from 'node:fs'
const sharp = createRequire(new URL('../web/package.json', import.meta.url))('sharp')
const dir = new URL('../web/public/shots/', import.meta.url).pathname
for (const f of fs.readdirSync(dir)) {
  if (!/^(board|calendar|doc|home)-(light|dark)-(en|fr)\.webp$/.test(f)) continue
  await sharp(dir + f).resize({ width: 900 }).webp({ quality: 80 }).toFile(dir + f.replace('.webp', '-900.webp'))
}
