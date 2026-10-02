import StarterKit from '@tiptap/starter-kit'
import Link from '@tiptap/extension-link'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Table from '@tiptap/extension-table'
import TableRow from '@tiptap/extension-table-row'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import Mention from '@tiptap/extension-mention'
import { ResizableImage } from './ResizableImage'
import { CustomMathExtension } from './CustomMathExtension'
import { CustomMathInlineExtension } from './CustomMathInlineExtension'
import { NoticeExtension } from './NoticeExtension'

/** Extensions communes à l'affichage et à l'édition (le schéma doit être identique). */
export function baseExtensions() {
  return [
    StarterKit.configure({ history: false }),
    Link.configure({ openOnClick: false, autolink: true, HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: '_blank' } }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    Table.configure({ resizable: true }),
    TableRow,
    TableHeader,
    TableCell,
    Mention.configure({ HTMLAttributes: { class: 'mention' } }),
    ResizableImage,
    CustomMathExtension,
    CustomMathInlineExtension,
    NoticeExtension,
  ]
}

/** Extensions pour un rendu en lecture seule. */
export function readonlyExtensions() {
  return baseExtensions()
}
