/**
 * CSS d'un SVG de Mermaid : rien ne doit pouvoir charger une ressource extérieure (pistage,
 * fuite d'adresse). On retire `@import` et tout `url(...)` sauf les références locales `url(#id)`
 * (flèches, dégradés), après avoir décodé les échappements et enlevé les commentaires.
 */
export function stripCssUrls(css: string): string {
  const decoded = css
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\\([0-9a-fA-F]{1,6})[ \t\n\r\f]?|\\([\s\S])/g, (_m, hex: string | undefined, ch: string | undefined) => {
      if (ch !== undefined) return ch === '\n' ? '' : ch
      const cp = parseInt(hex!, 16)
      return cp > 0 && cp <= 0x10ffff ? String.fromCodePoint(cp) : ''
    })
  return decoded
    .replace(/@import\b[^;]*(;|$)/gi, '')
    .replace(/url\s*\(\s*(?!['"]?\s*#)[^)]*\)?/gi, 'none')
    .replace(/(?:expression|image-set|-webkit-image-set|image)\s*\(/gi, 'none(')
}
