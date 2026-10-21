// Jetons fictifs du faux Moodle, dérivés d'un texte pour rester stables d'une exécution à l'autre.
import { createHash } from 'node:crypto'
const derive = (name) => createHash('sha256').update(`looty-e2e-${name}`).digest('hex').slice(0, 32)
export const MOODLE_TOKEN = derive('token')
export const PRIVATE_TOKEN = derive('private')
