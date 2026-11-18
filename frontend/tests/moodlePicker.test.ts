// node --test frontend/tests/moodlePicker.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { courseEntries, fileEntries, courseKey, fileKey, type PickerCourse, type CourseSection } from '../app/utils/moodlePicker.ts'
import { groupByKind, filterHits, matchesFilter, flatten, GROUP_ORDER } from '../app/utils/pickerSearch.ts'

const courses: PickerCourse[] = [
  { id: 1, fullname: 'Algèbre linéaire', shortname: 'ALG', category: 'Mathématiques', favourite: true },
  { id: 2, fullname: 'Systèmes d’exploitation', shortname: 'OS', category: 'Informatique' },
  { id: 3, fullname: 'Histoire (archive)', hidden: true },
]
const sections: CourseSection[] = [
  { id: 10, name: 'Semaine 1', modules: [
    { id: 1, name: 'Polycopié', type: 'resource', type_name: 'Fichiers', files: [{ name: 'poly.pdf', size: 2_000_000, mime: 'application/pdf', source: 'https://m.test/pluginfile.php/1/poly.pdf' }] },
    { id: 2, name: 'Diaporama', type: 'resource', files: [{ name: 'cours.pptx', size: 5000, mime: null, source: 'https://m.test/pluginfile.php/1/cours.pptx' }, { name: 'logo.png', size: 10, mime: 'image/png', source: 'https://m.test/pluginfile.php/1/logo.png' }] },
  ] },
  { id: 11, name: 'Examens', modules: [
    { id: 3, name: 'Sujet', type: 'folder', type_name: 'Dossiers', files: [{ name: 'sujet.docx', size: 40_000, mime: null, source: 'https://m.test/pluginfile.php/1/sujet.docx' }, { name: 'index.html', size: 10, mime: 'text/html', html: true, source: 'https://m.test/pluginfile.php/1/index.html' }] },
  ] },
]

test('les cours de la connexion sont proposés, sauf les masqués tant qu’on ne les cherche pas', () => {
  const all = courseEntries('w1', 'Fac', courses, '')
  assert.deepEqual(all.map((e) => e.courseId), [1, 2])
  assert.equal(all[0]!.kind, 'moodle_course')
  assert.equal(all[0]!.key, courseKey('w1', 1))
  assert.equal(all[0]!.place, 'Mathématiques')
  // Avec plusieurs espaces, l'espace précède la catégorie.
  assert.equal(courseEntries('w1', 'Fac', courses, '', true)[0]!.place, 'Fac › Mathématiques')
})

test('la recherche de cours ignore casse et accents et couvre nom court et catégorie', () => {
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'ALGEBRE').map((e) => e.courseId), [1])
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'systemes').map((e) => e.courseId), [2])
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'informatique').map((e) => e.courseId), [2])
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'os').map((e) => e.courseId), [2])
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'archive').map((e) => e.courseId), [3], 'un cours masqué se trouve en le cherchant')
  assert.deepEqual(courseEntries('w1', 'Fac', courses, 'zzz'), [])
})

test('les fichiers d’un cours : seuls les formats lisibles, avec section, type et taille pour l’aperçu', () => {
  const files = fileEntries('w1', { id: 1, name: 'Algèbre' }, sections, '')
  assert.deepEqual(files.map((e) => e.title), ['poly.pdf', 'cours.pptx', 'sujet.docx'])
  const poly = files[0]!
  assert.equal(poly.kind, 'moodle_file')
  assert.equal(poly.key, fileKey('w1', 'https://m.test/pluginfile.php/1/poly.pdf'))
  assert.deepEqual([poly.section, poly.type, poly.size, poly.courseId, poly.courseName], ['Semaine 1', 'Fichiers', 2_000_000, 1, 'Algèbre'])
  assert.equal(poly.place, 'Algèbre › Semaine 1')
  assert.equal(files[1]!.type, 'resource', 'sans nom de type, le type de l’activité')
})

test('la recherche dans un cours porte sur le fichier, l’activité et la section', () => {
  const find = (q: string) => fileEntries('w1', { id: 1, name: 'A' }, sections, q).map((e) => e.title)
  assert.deepEqual(find('POLY'), ['poly.pdf'])
  assert.deepEqual(find('diaporama'), ['cours.pptx'])
  assert.deepEqual(find('examens'), ['sujet.docx'])
  assert.deepEqual(find('rien'), [])
})

test('Moodle forme ses propres groupes, après les autres, et le filtre Moodle couvre cours et fichiers', () => {
  const mix = [
    ...fileEntries('w1', { id: 1, name: 'A' }, sections, '').slice(0, 1),
    { kind: 'page', key: 'p', id: 'p', ws: 'w1', title: 'Page', place: '', excerpt: '' },
    ...courseEntries('w1', 'Fac', courses, '').slice(0, 1),
    { kind: 'file', key: 'f', id: 'f', ws: 'w1', title: 'drive.pdf', place: '', excerpt: '' },
  ]
  assert.deepEqual(groupByKind(mix).map((s) => s.id), ['page', 'file', 'moodle_course', 'moodle_file'])
  assert.deepEqual(GROUP_ORDER.slice(-2), ['moodle_course', 'moodle_file'])
  assert.deepEqual(groupByKind(mix, 'moodle').map((s) => s.id), ['moodle_course', 'moodle_file'])
  assert.deepEqual(groupByKind(mix, 'page').map((s) => s.id), ['page'])
  assert.equal(filterHits(mix, 'moodle').length, 2)
  assert.deepEqual(flatten(groupByKind(mix, 'moodle')).map((e) => e.kind), ['moodle_course', 'moodle_file'])
  assert.ok(matchesFilter('moodle_file', 'moodle') && matchesFilter('moodle_course', 'all') && !matchesFilter('file', 'moodle') && !matchesFilter('moodle_file', 'file'))
})
