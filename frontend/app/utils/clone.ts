/**
 * Copie profonde de données JSON (réponses d'API). `structuredClone` échoue sur
 * les proxys réactifs imbriqués de Vue ; nos données étant du JSON pur, ceci suffit.
 */
export function clone<T>(value: T): T {
  return value === undefined ? value : JSON.parse(JSON.stringify(value))
}
