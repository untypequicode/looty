import type { Component } from 'vue'
import { FileText, FileType, FileCode, FileSpreadsheet, Presentation } from 'lucide-vue-next'
import { fileKind, type FileKind } from './chatFiles'

const ICONS: Record<FileKind, Component> = { pdf: FileType, doc: FileText, slides: Presentation, data: FileSpreadsheet, code: FileCode, text: FileText }

/** Icône d'un fichier joint selon son format (PDF, Word, présentation, données, code, texte). */
export const fileIcon = (name: string): Component => ICONS[fileKind(name)]
