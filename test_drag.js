const targetPage = { id: 'B', parent_id: null, position: 10 }
const draggedPage = { id: 'A', parent_id: null, position: 5 }
const dropTarget = { id: 'B', position: 'inside' }
const props = { pages: [targetPage, draggedPage] }

let parentId = targetPage.parent_id
let newPosition = targetPage.position || 0

if (dropTarget.position === 'inside') {
  parentId = targetPage.id
  const children = props.pages.filter(p => p.parent_id === targetPage.id)
  const lastChild = children.sort((a, b) => (b.position || 0) - (a.position || 0))[0]
  newPosition = lastChild ? (lastChild.position || 0) + 1000 : 1000
} else if (dropTarget.position === 'before') {
  newPosition = (targetPage.position || 0) - 1000
} else if (dropTarget.position === 'after') {
  newPosition = (targetPage.position || 0) + 1000
}

console.log({ parentId, newPosition })
