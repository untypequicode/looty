/**
 * Lien de partage d'un jeu de révision (`/study/<id>?token=…`) : la page de connexion l'explique,
 * et le chemin complet (jeton compris) est rouvert après la connexion (paramètre `redirect`).
 * Les tentatives restent propres à chaque personne : le jeton ne remplace jamais la connexion.
 */
export function isStudyShareRedirect(redirect: unknown): boolean {
  if (typeof redirect !== 'string' || !redirect.startsWith('/study/') || redirect.startsWith('//')) return false
  const [path, query = ''] = redirect.split('#')[0]!.split('?')
  if (!/^\/study\/[^/]+\/?$/.test(path!)) return false
  return new URLSearchParams(query).has('token')
}
