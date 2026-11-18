import DOMPurify from 'dompurify'
import { stripCssUrls } from './svgCss.ts'

/**
 * SVG de Mermaid (mode strict) nettoyé une seconde fois : ni script, ni objet ou cadre incorporé, ni lien,
 * et un CSS qui ne charge rien d'extérieur. Module à part (sans dépendance lourde) pour être testé avec un DOM.
 */
export function sanitizeSvg(svg: string): string {
  const frag = DOMPurify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true, html: true },
    ADD_TAGS: ['foreignobject'],
    ADD_ATTR: ['dominant-baseline'],
    FORBID_TAGS: ['script', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'a'],
    FORBID_ATTR: ['onload', 'onclick', 'onerror'],
    RETURN_DOM_FRAGMENT: true,
  }) as DocumentFragment
  // Le CSS ne charge rien d'extérieur : ni `@import`, ni `url(http…)` (balises <style> et attributs style).
  frag.querySelectorAll('style').forEach((s) => { s.textContent = stripCssUrls(s.textContent ?? '') })
  frag.querySelectorAll('[style]').forEach((el) => el.setAttribute('style', stripCssUrls(el.getAttribute('style') ?? '')))
  const box = document.createElement('div')
  box.append(frag)
  return box.innerHTML
}
