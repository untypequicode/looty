/**
 * Où part un envoi de fichier : la page ouverte (images et pièces de l'éditeur, voir ADR 0022 phase 5) ou,
 * à défaut, l'espace. Le serveur rattache le fichier à cette page : qui peut la lire peut lire le fichier,
 * et retirer l'accès à la page coupe aussi le fichier.
 */
const UUID = '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'

export function uploadContext(pathname: string): { page?: string; workspace?: string } {
  const page = new RegExp(`^/(?:w/${UUID}/(?:doc|read)|shared)/(${UUID})(?:/|$)`).exec(pathname)
  if (page) return { page: page[1] }
  const ws = new RegExp(`^/w/(${UUID})(?:/|$)`).exec(pathname)
  if (ws) return { workspace: ws[1] }
  return {}
}

/** `/api/upload` avec le contexte de la page courante. */
export function uploadUrl(base: string, pathname: string, personal = false): string {
  if (base !== '/api/upload') return base
  // Bloc-notes : jamais lié à la page ouverte, lisible par son auteur seulement.
  if (personal) return `${base}?personal=1`
  const ctx = uploadContext(pathname)
  if (ctx.page) return `${base}?page=${ctx.page}`
  if (ctx.workspace) return `${base}?workspace=${ctx.workspace}`
  return base
}
