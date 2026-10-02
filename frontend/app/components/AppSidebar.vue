<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useColorMode } from '@vueuse/core'
import type { User, Workspace, Board, Page } from '../types'
import PageTree from './PageTree.vue'
import { marked, Marked } from 'marked'
import JSZip from 'jszip'
import TurndownService from 'turndown'

const props = defineProps<{
  user: { authenticated: boolean, user?: User }
  workspacesData?: { workspaces: Workspace[] }
  isLoadingWorkspaces: boolean
  activeWorkspaceId: string | null
  activeWorkspaceRole: string | null
  boards: Board[]
  pages: Page[]
  isLoadingPages: boolean
  activeBoard: Board | null
  activePage: Page | null
  currentModule: 'kanban' | 'wiki'
}>()

const emit = defineEmits<{
  (e: 'select-workspace', id: string): void
  (e: 'select-board', board: Board): void
  (e: 'select-page', page: Page): void
  (e: 'open-settings'): void
  (e: 'open-rename', type: 'workspace' | 'board' | 'column', id: string, name: string): void
  (e: 'delete-workspace', id: string): void
  (e: 'delete-board', id: string): void
  (e: 'delete-page', id: string): void
  (e: 'create-page', title?: string, content?: string): void
  (e: 'create-subpage', id: string, title?: string, content?: string): void
  (e: 'show-create-modal'): void
  (e: 'show-board-modal'): void
  (e: 'copy-invite-link'): void
  (e: 'move-page', payload: any): void
}>()

const getInitials = (name: string) => name ? name.charAt(0).toUpperCase() : '?'
const colorMode = ref('auto')

const applyTheme = () => {
  if (typeof document === 'undefined') return
  if (colorMode.value === 'dark' || (colorMode.value === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    document.documentElement.classList.add('dark')
    document.documentElement.classList.remove('light')
  } else {
    document.documentElement.classList.remove('dark')
    document.documentElement.classList.add('light')
  }
}

onMounted(() => {
  const saved = localStorage.getItem('theme-mode')
  if (saved) colorMode.value = saved
  applyTheme()

  if (typeof window !== 'undefined') {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
      if (colorMode.value === 'auto') applyTheme()
    })
  }
})

const toggleTheme = () => {
  if (colorMode.value === 'auto') colorMode.value = 'light'
  else if (colorMode.value === 'light') colorMode.value = 'dark'
  else colorMode.value = 'auto'
  
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('theme-mode', colorMode.value)
  }
  applyTheme()
}

const showSearch = ref(false)
const searchQuery = ref('')
const searchResults = ref([])
const isSearching = ref(false)
const selectedIndex = ref(0)
const fileInput = ref<HTMLInputElement | null>(null)

const folderInput = ref<HTMLInputElement | null>(null)

const isProcessingZip = ref(false)
const zipProgress = ref({ current: 0, total: 0, message: '' })

const triggerImportFiles = () => {
  if (fileInput.value) fileInput.value.click()
}
const triggerImportFolder = () => {
  if (folderInput.value) folderInput.value.click()
}

const handleImportMarkdown = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const filesArray = Array.from(target.files || [])
  if (filesArray.length === 0) return

  const markdownFiles = filesArray.filter(f => f.name.endsWith('.md') || f.name.endsWith('.markdown'))
  const imageFiles = filesArray.filter(f => !f.name.endsWith('.md') && !f.name.endsWith('.markdown'))

  const uploadCache: Record<string, string> = {}

  async function resolveImage(href: string, mdFile: File) {
    if (href.startsWith('http')) return href

    let mdDir = ''
    if (mdFile.webkitRelativePath) {
      const parts = mdFile.webkitRelativePath.split('/')
      parts.pop()
      if (parts.length > 0) mdDir = parts.join('/') + '/'
    }

    const targetPath = mdDir + href

    let matchedFile = imageFiles.find(f => {
      let p1 = f.webkitRelativePath || f.name
      let p2 = targetPath.replace(/^\.\//, '')
      return p1.endsWith(p2) || p2.endsWith(p1)
    })

    if (!matchedFile) {
      const basename = href.split('/').pop()
      if (basename) {
        matchedFile = imageFiles.find(f => f.name === basename)
      }
    }

    if (matchedFile) {
      const cacheKey = matchedFile.webkitRelativePath || matchedFile.name
      if (uploadCache[cacheKey]) return uploadCache[cacheKey]

      const formData = new FormData()
      formData.append('file', matchedFile)
      try {
        const res = await fetch('/api/upload', { method: 'POST', body: formData })
        const data = await res.json()
        if (data.url) {
          uploadCache[cacheKey] = data.url
          return data.url
        }
      } catch (e) {
        console.error('Failed to upload image', e)
      }
    }
    return href
  }

  const mathBlockExtension = {
    name: 'math-block',
    level: 'block',
    start(src: string) { return src.match(/\$\$/)?.index; },
    tokenizer(src: string, tokens: any) {
      const rule = /^\$\$([^$]+)\$\$/;
      const match = rule.exec(src);
      if (match) {
        return {
          type: 'math-block',
          raw: match[0],
          text: match[1].trim()
        };
      }
    },
    renderer(token: any) {
      const escapedLatex = token.text.replace(/"/g, '&quot;');
      return `<div data-type="math-block" latex="${escapedLatex}"></div>`;
    }
  };

  const mathInlineExtension = {
    name: 'math-inline',
    level: 'inline',
    start(src: string) { return src.match(/\$/)?.index; },
    tokenizer(src: string, tokens: any) {
      const rule = /^\$([^$]+)\$/;
      const match = rule.exec(src);
      if (match) {
        return {
          type: 'math-inline',
          raw: match[0],
          text: match[1].trim()
        };
      }
    },
    renderer(token: any) {
      const escapedLatex = token.text.replace(/"/g, '&quot;');
      return `<span data-type="math-inline" latex="${escapedLatex}"></span>`;
    }
  };

  let currentMdFile: File | null = null;
  const markedInstance = new Marked();
  markedInstance.use({ extensions: [mathBlockExtension, mathInlineExtension] });
  markedInstance.use({
    async walkTokens(token: any) {
      if (token.type === 'image' && currentMdFile) {
        token.href = await resolveImage(token.href, currentMdFile);
      }
    }
  });

  for (let i = 0; i < markdownFiles.length; i++) {
    const file = markdownFiles[i]
    currentMdFile = file
    const text = await file.text()
    const html = await markedInstance.parse(text, { async: true })
    const title = file.name.replace(/\.md$/i, '')
    
    emit('create-page', title, html)
  }
  target.value = ''
}

const zipInput = ref<HTMLInputElement | null>(null)
const triggerImportZip = () => {
  if (zipInput.value) zipInput.value.click()
}

const createMarkedInstance = () => {
  const mathBlockExtension = {
    name: 'math-block',
    level: 'block',
    start(src: string) { return src.match(/\$\$/)?.index; },
    tokenizer(src: string, tokens: any) {
      const rule = /^\$\$([^$]+)\$\$/;
      const match = rule.exec(src);
      if (match) {
        return { type: 'math-block', raw: match[0], text: match[1].trim() };
      }
    },
    renderer(token: any) {
      const escapedLatex = token.text.replace(/"/g, '&quot;');
      return `<div data-type="math-block" latex="${escapedLatex}"></div>`;
    }
  };
  const mathInlineExtension = {
    name: 'math-inline',
    level: 'inline',
    start(src: string) { return src.match(/\$/)?.index; },
    tokenizer(src: string, tokens: any) {
      const rule = /^\$([^$]+)\$/;
      const match = rule.exec(src);
      if (match) {
        return { type: 'math-inline', raw: match[0], text: match[1].trim() };
      }
    },
    renderer(token: any) {
      const escapedLatex = token.text.replace(/"/g, '&quot;');
      return `<span data-type="math-inline" latex="${escapedLatex}"></span>`;
    }
  };
  const noticeExtension = {
    name: 'notice',
    level: 'block',
    start(src: string) { return src.match(/^:::[a-z]+\n/)?.index; },
    tokenizer(src: string) {
      const rule = /^:::([a-z]+)\n([\s\S]*?)\n:::/;
      const match = rule.exec(src);
      if (match) {
        return {
          type: 'notice',
          raw: match[0],
          noticeType: match[1],
          text: match[2].trim(),
          tokens: this.lexer.blockTokens(match[2].trim())
        };
      }
    },
    renderer(token: any) {
      // For now, render as a blockquote with a special class.
      // A full Tiptap extension will be added to WysiwygEditor to parse this properly.
      return `<blockquote data-type="notice" data-notice-type="${token.noticeType}">${this.parser.parse(token.tokens)}</blockquote>\n`;
    }
  };
  const inst = new Marked();
  inst.use({ extensions: [mathBlockExtension, mathInlineExtension, noticeExtension] });
  return inst;
}

const handleImportZip = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  const zip = await JSZip.loadAsync(file)
  const uploadCache: Record<string, string> = {}

  // Upload a file from zip and return the new URL
  async function uploadZipFile(zipEntry: any): Promise<string> {
    const path = zipEntry.name
    if (uploadCache[path]) return uploadCache[path]
    
    const blob = await zipEntry.async('blob')
    const ext = path.split('.').pop() || 'bin'
    const filename = path.split('/').pop() || 'file'
    const fileObj = new File([blob], filename, { type: `image/${ext}` })
    const formData = new FormData()
    formData.append('file', fileObj)
    try {
      const res = await fetch('/api/upload', { method: 'POST', body: formData })
      const data = await res.json()
      if (data.url) {
        uploadCache[path] = data.url
        return data.url
      }
    } catch (e) {
      console.error('Failed to upload from zip', path, e)
    }
    return path
  }

  // Build a tree from the zip file paths
  // Structure: folder = page with children, .md = page content
  interface PageNode {
    name: string
    mdPath?: string
    children: PageNode[]
    dirPath: string
  }

  const allPaths = Object.keys(zip.files)
  const mdFiles = allPaths.filter(p => p.endsWith('.md') && !zip.files[p].dir)
  
  // Build hierarchy
  // Outline structure: Dir/Page.md + Dir/Page/SubPage.md
  const rootNodes: PageNode[] = []
  const nodeMap: Record<string, PageNode> = {}

  // Sort md files by depth (shallowest first)
  mdFiles.sort((a, b) => a.split('/').length - b.split('/').length)

  for (const mdPath of mdFiles) {
    const parts = mdPath.split('/')
    const filename = parts.pop()!
    const dirPath = parts.join('/')
    const name = filename.replace(/\.md$/i, '')

    const node: PageNode = { name, mdPath, children: [], dirPath }
    nodeMap[mdPath] = node

    // Check if there's a parent md at the level above
    // e.g., for CMI/S7/Programmation Linéaire/TDs/TD2.md,
    //   parent dir = CMI/S7/Programmation Linéaire/TDs
    //   parent md = CMI/S7/Programmation Linéaire/TDs.md
    if (parts.length >= 2) {
      const parentDir = parts.slice(0, -1).join('/')
      const parentName = parts[parts.length - 1]
      const parentMdPath = parentDir + '/' + parentName + '.md'
      
      if (nodeMap[parentMdPath]) {
        nodeMap[parentMdPath].children.push(node)
        continue
      }
    }

    // Check if it's a top-level page (depth 2: Collection/Page.md)
    if (parts.length <= 1) {
      rootNodes.push(node)
    } else {
      // Try to find any ancestor
      let placed = false
      for (let i = parts.length - 1; i >= 1; i--) {
        const ancestorDir = parts.slice(0, i).join('/')
        const ancestorName = parts[i - 1] === parts[0] && i === 1 ? parts[0] : parts[i]
        const tryPath = parts.slice(0, i).join('/') + '.md'
        // Try: parent-dir-name.md in grandparent
        const tryPath2 = (i >= 2 ? parts.slice(0, i - 1).join('/') + '/' : '') + parts[i - 1] + '.md'
        
        if (nodeMap[tryPath]) {
          nodeMap[tryPath].children.push(node)
          placed = true
          break
        }
        if (nodeMap[tryPath2]) {
          nodeMap[tryPath2].children.push(node)
          placed = true
          break
        }
      }
      if (!placed) {
        rootNodes.push(node)
      }
    }
  }

  const markedInstance = createMarkedInstance()

  // Process images in markdown text
  async function processMarkdown(text: string, basePath: string): Promise<string> {
    // Find all image references and upload them
    const imgRegex = /!\[([^\]]*)\]\(([^)]+)\)/g
    let result = text
    const matches = [...text.matchAll(imgRegex)]
    
    for (const match of matches) {
      const rawUrl = match[2].split('"')[0].trim()  // Remove title part like ' " =100x100"'
      if (rawUrl.startsWith('http')) continue
      
      // Try to find the file in the zip
      const tryPaths = [
        basePath + '/' + rawUrl,
        rawUrl,
        basePath + '/' + rawUrl.replace(/^\.\//,'')
      ].map(p => p.replace(/^\//, ''))
      
      for (const tryPath of tryPaths) {
        const zipEntry = zip.files[tryPath]
        if (zipEntry && !zipEntry.dir) {
          const newUrl = await uploadZipFile(zipEntry)
          result = result.replace(match[0], `![${match[1]}](${newUrl})`)
          break
        }
      }
    }
    return result
  }

  // Recursively create pages using direct API calls to get page IDs back
  async function createPageTree(node: PageNode, parentId?: string) {
    let mdText = ''
    if (node.mdPath) {
      const zipEntry = zip.files[node.mdPath]
      if (zipEntry) {
        mdText = await zipEntry.async('string')
      }
    }
    
    let title = node.name
    let icon = null
    mdText = mdText.trimStart()
    const firstLineMatch = mdText.match(/^#\s+(?:([\p{Emoji_Presentation}\p{Extended_Pictographic}])\s+)?(.*)/u)
    if (firstLineMatch) {
      icon = firstLineMatch[1] || null
      title = firstLineMatch[2] || node.name
      mdText = mdText.replace(/^#.*\n?/, '') // remove the title line
    }

    const processedMd = await processMarkdown(mdText, node.dirPath)
    const html = await markedInstance.parse(processedMd, { async: false })
    
    zipProgress.value.message = `Importation de : ${title}`
    zipProgress.value.current++
    
    try {
      const createdPage = await $fetch(`/api/workspaces/${props.activeWorkspaceId}/pages`, {
        method: 'POST',
        body: { title, content: html, parent_id: parentId || null, icon }
      }) as any
      
      // Recursively create children with this page as parent
      for (const child of node.children) {
        await createPageTree(child, createdPage.id)
      }
    } catch (e) {
      console.error('Failed to create page', title, e)
    }
  }

  isProcessingZip.value = true
  zipProgress.value = { current: 0, total: mdFiles.length, message: 'Analyse du ZIP...' }

  try {
    for (const rootNode of rootNodes) {
      await createPageTree(rootNode)
    }
    emit('create-page') // refresh
  } finally {
    isProcessingZip.value = false
    target.value = ''
  }
}

// Export full workspace as ZIP
const exportWorkspaceZip = async () => {
  if (!props.activeWorkspaceId) return
  
  const zip = new JSZip()
  const turndownService = new TurndownService({
    headingStyle: 'atx',
    codeBlockStyle: 'fenced',
    blankReplacement: function (content: string, node: any) {
      if (node.nodeName === 'SPAN' && node.getAttribute('data-type') === 'math-inline') {
        return '$' + (node.getAttribute('latex') || '') + '$'
      }
      if (node.nodeName === 'DIV' && node.getAttribute('data-type') === 'math-block') {
        return '\n$$\n' + (node.getAttribute('latex') || '') + '\n$$\n'
      }
      return node.isBlock ? '\n\n' : ''
    }
  })
  turndownService.addRule('task-list', {
    filter: (node: any) => node.nodeName === 'LI' && node.getAttribute('data-type') === 'taskItem',
    replacement: (content: string, node: any) => {
      const isChecked = node.hasAttribute('data-checked')
      return (isChecked ? '- [x] ' : '- [ ] ') + content + '\n'
    }
  })
  turndownService.addRule('notice', {
    filter: (node: any) => node.getAttribute('data-type') === 'notice',
    replacement: (content: string, node: any) => {
      const type = node.getAttribute('data-notice-type') || 'info'
      return `\n:::${type}\n${content.trim()}\n:::\n\n`
    }
  })

  isProcessingZip.value = true
  zipProgress.value = { current: 0, total: props.pages.length, message: 'Préparation...' }
  
  async function addPageToZip(page: any, dirPath: string) {
    let fullPage = page
    try {
      fullPage = await $fetch(`/api/workspaces/${props.activeWorkspaceId}/pages/${page.id}`)
    } catch (e) {
      console.error('Failed to fetch page', page.id)
    }
    
    zipProgress.value.message = `Exportation de : ${fullPage.title}`
    zipProgress.value.current++
    
    let markdown = turndownService.turndown(fullPage.content || '')
    
    // Add Outline title
    markdown = `# ${fullPage.icon ? fullPage.icon + ' ' : ''}${fullPage.title || 'Untitled'}\n\n` + markdown
    
    // Download and include images
    const imgRegex = /!\[([^\]]*)\]\(([^)]+)\)/g
    const matches = [...markdown.matchAll(imgRegex)]
    for (const match of matches) {
      const url = match[2].split(' ')[0]
      if (url.startsWith('/uploads/') || url.startsWith('uploads/')) {
        try {
          const resp = await fetch(url.startsWith('/') ? url : '/' + url)
          if (resp.ok) {
            const blob = await resp.blob()
            const filename = url.split('/').pop()!
            zip.file(dirPath + 'uploads/' + filename, blob)
            markdown = markdown.replaceAll(url, 'uploads/' + filename)
          }
        } catch (e) { /* skip */ }
      }
    }
    
    const safeName = (fullPage.title || 'Untitled').replace(/[/\\?%*:|"<>]/g, '-')
    zip.file(dirPath + safeName + '.md', markdown)
    
    const children = fullPage.children || []
    if (children.length > 0) {
      for (const child of children) {
        await addPageToZip(child, dirPath + safeName + '/')
      }
    }
  }

  try {
    for (const page of props.pages) {
      await addPageToZip(page, '')
    }

    zipProgress.value.message = 'Compression...'
    const workspaceName = props.workspacesData?.workspaces?.find(w => w.id === props.activeWorkspaceId)?.name || 'workspace'
    const content = await zip.generateAsync({ type: 'blob' })
    const url = URL.createObjectURL(content)
    const a = document.createElement('a')
    a.href = url
    a.download = `${workspaceName}.zip`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  } finally {
    isProcessingZip.value = false
  }
}

const onSearchKeyDown = (e) => {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    if (selectedIndex.value < searchResults.value.length - 1) selectedIndex.value++
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    if (selectedIndex.value > 0) selectedIndex.value--
  } else if (e.key === 'Enter') {
    e.preventDefault()
    if (searchResults.value.length > 0 && searchResults.value[selectedIndex.value]) {
      selectSearchResult(searchResults.value[selectedIndex.value])
    }
  } else if (e.key === 'Escape') {
    showSearch.value = false
  }
}

const handleGlobalKeydown = (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    if (props.activeWorkspaceId) {
      showSearch.value = true
      nextTick(() => {
        const input = document.querySelector('.spotlight-input')
        if (input) input.focus()
      })
    }
  }
}

onMounted(() => {
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleGlobalKeydown)
  }
})

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', handleGlobalKeydown)
  }
})

const handleSearch = async () => {
  if (!searchQuery.value || searchQuery.value.trim().length < 2) {
    searchResults.value = []
    return
  }
  isSearching.value = true
  try {
    const res = await $fetch(`/api/workspaces/${props.activeWorkspaceId}/search?q=${encodeURIComponent(searchQuery.value)}`)
    searchResults.value = res.results || []
    selectedIndex.value = 0
  } catch (e) {
    console.error("Search error", e)
  }
  isSearching.value = false
}

const selectSearchResult = (result) => {
  if (result.type === 'page') {
    const findPage = (pages, id) => {
      for (const p of pages) {
        if (p.id === id) return p
        if (p.children && p.children.length > 0) {
          const found = findPage(p.children, id)
          if (found) return found
        }
      }
      return null
    }
    const p = findPage(props.pages || [], result.id)
    if (p) {
      emit('select-page', p)
    } else {
      emit('select-page', { id: result.id, title: result.title })
    }
  } else if (result.type === 'task') {
    // Navigate to the board and open the task
    // It emits an event to index.vue to handle it
    emit('open-task-from-search', result)
  }
  showSearch.value = false
  searchQuery.value = ''
  searchResults.value = []
  selectedIndex.value = 0
}

let searchTimeout = null
const onSearchInput = () => {
  clearTimeout(searchTimeout)
  searchTimeout = setTimeout(handleSearch, 300)
}

const favoritePages = computed(() => {
  return (props.pages || []).filter(p => p.is_favorite)
})
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header"><div class="logo">Looty</div></div>
    
    <div class="sidebar-content">
      <div v-if="activeWorkspaceId" class="sidebar-search" style="padding: 1rem 0 0 0;">
        <button @click="showSearch = true" class="search-btn" style="width: 100%; display: flex; align-items: center; gap: 8px; background: var(--bg-color-3); border: 1px solid var(--border-color); padding: 8px 12px; border-radius: var(--radius-md); color: var(--text-color-3); cursor: pointer; transition: all 0.2s;">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
          <span>Rechercher...</span>
        </button>
      </div>

      <!-- Search Modal -->
      <ClientOnly>
        <Teleport to="body">
          <div v-if="showSearch" class="modal-overlay spotlight-overlay" @click.self="showSearch = false">
            <div class="spotlight-modal">
              <div class="spotlight-input-wrapper">
                <svg class="spotlight-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
                <input 
                  v-model="searchQuery" 
                  @input="onSearchInput"
                  @keydown="onSearchKeyDown"
                  type="text" 
                  placeholder="Rechercher des documents, pages, tâches..." 
                  class="spotlight-input" 
                  autofocus
                />
                <div class="spotlight-shortcut">ESC</div>
              </div>
              <div class="spotlight-results" v-if="searchQuery.length > 0">
                <div v-if="isSearching" class="spotlight-state">
                  Recherche...
                </div>
                <div v-else-if="searchResults.length === 0 && searchQuery.length >= 2" class="spotlight-state">
                  Aucun résultat pour "<span style="color: var(--text-color);">{{ searchQuery }}</span>"
                </div>
                <div v-else-if="searchQuery.length < 2" class="spotlight-state">
                  Tapez au moins 2 lettres...
                </div>
                <div v-else class="spotlight-list">
                  <div 
                    v-for="(res, index) in searchResults" 
                    :key="res.id"
                    @click="selectSearchResult(res)"
                    @mouseover="selectedIndex = index"
                    class="spotlight-item"
                    :class="{ 'is-selected': selectedIndex === index }"
                  >
                    <div class="spotlight-item-icon">
                      <svg v-if="res.type === 'page'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" /></svg>
                      <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" /></svg>
                    </div>
                    <div class="spotlight-item-content">
                      <div class="spotlight-item-title">{{ res.title }}</div>
                      <div class="spotlight-item-snippet" v-if="res.snippet">{{ res.snippet.replace(/<[^>]*>?/gm, '') }}...</div>
                    </div>
                    <div class="spotlight-item-arrow" v-if="selectedIndex === index">
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Teleport>
      </ClientOnly>

      <div class="section-title">
        <span>Espaces de Travail</span>
        <div style="display: flex; gap: 0.5rem;">
          <button v-if="activeWorkspaceId" @click="emit('copy-invite-link')" class="icon-btn" title="Inviter des membres">🔗</button>
          <button @click="emit('show-create-modal')" class="icon-btn" title="Créer un espace">+</button>
        </div>
      </div>
      
      <ul class="workspace-list">
        <li v-if="isLoadingWorkspaces" class="loading-item">Chargement...</li>
        <li v-else-if="!workspacesData?.workspaces?.length" class="empty-item">Aucun espace</li>
        <li v-for="ws in workspacesData?.workspaces" :key="ws.id"
          :class="['workspace-item', { active: activeWorkspaceId === ws.id }]" 
          @click="emit('select-workspace', ws.id)">
          <div style="display: flex; align-items: center; gap: 10px; flex: 1">
            <div class="ws-avatar">{{ getInitials(ws.name) }}</div>
            <span class="ws-name">{{ ws.name }}</span>
          </div>
          <div class="item-actions" v-if="activeWorkspaceId === ws.id">
            <button @click.stop="emit('open-settings')" title="Paramètres (Membres)">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>
            </button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('open-rename', 'workspace', ws.id, ws.name)" title="Renommer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
            </button>
            <button v-if="ws.role === 'owner' || ws.role === 'admin'" @click.stop="emit('delete-workspace', ws.id)" title="Supprimer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            </button>
          </div>
        </li>
      </ul>

    <div v-if="activeWorkspaceId" class="section-title" style="margin-top:1rem;">
      <span>Général</span>
    </div>
    <ul v-if="activeWorkspaceId" class="workspace-list">
      <li :class="['workspace-item', { active: currentModule === 'dashboard' }]" @click="emit('go-dashboard')">
        <span class="ws-name" style="flex: 1; display: flex; align-items: center; gap: 6px;">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" class="item-icon"><path stroke-linecap="round" stroke-linejoin="round" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"/></svg>
          Accueil
        </span>
      </li>
    </ul>

    <div v-if="activeWorkspaceId" class="section-title" style="margin-top:1rem;">
      <span>Favoris</span>
    </div>
      <div v-if="activeWorkspaceId && favoritePages.length === 0" class="empty-item">Aucun favori</div>
      <PageTree
        v-if="activeWorkspaceId && favoritePages.length > 0"
        :pages="pages"
        :root-ids="favoritePages.map(p => p.id)"
        :level="0"
        :active-page="activePage"
        :current-module="currentModule"
        @select-page="emit('select-page', $event)"
        @delete-page="emit('delete-page', $event)"
        @create-subpage="emit('create-subpage', $event)"
        @move-page="emit('move-page', $event)"
      />

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Tableaux</span>
        <button v-if="activeWorkspaceRole !== 'viewer'" @click="emit('show-board-modal')" class="icon-btn" title="Créer un tableau">+</button>
      </div>
      
      <ul v-if="activeWorkspaceId" class="workspace-list">
        <li v-if="boards.length === 0" class="empty-item">Aucun tableau</li>
        <li v-for="b in boards" :key="b.id"
          :class="['workspace-item', { active: activeBoard?.id === b.id && currentModule === 'kanban' }]" 
          @click="emit('select-board', b)">
          <span class="ws-name" style="flex: 1; display: flex; align-items: center; gap: 6px;">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" class="item-icon"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><path d="M9 3v18M15 3v18M3 9h18M3 15h18"/></svg>
            {{ b.name }}
          </span>
          <div class="item-actions" v-if="activeBoard?.id === b.id && activeWorkspaceRole !== 'viewer'">
            <button @click.stop="emit('open-rename', 'board', b.id, b.name)" title="Renommer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>
            </button>
            <button @click.stop="emit('delete-board', b.id)" title="Supprimer">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            </button>
          </div>
        </li>
      </ul>

      <div v-if="activeWorkspaceId" class="section-title" style="margin-top:2rem;">
        <span>Wiki (Pages)</span>
        <div style="display: flex; gap: 0.5rem;" v-if="!isProcessingZip">
          <button @click="triggerImportFiles" class="icon-btn" title="Importer fichier(s) Markdown">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" /></svg>
          </button>
          <input type="file" ref="fileInput" multiple style="display: none;" @change="handleImportMarkdown" />
          <button @click="triggerImportFolder" class="icon-btn" title="Importer un dossier entier (avec images)">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" /></svg>
          </button>
          <input type="file" ref="folderInput" webkitdirectory directory multiple style="display: none;" @change="handleImportMarkdown" />
          <button @click="triggerImportZip" class="icon-btn" title="Importer un ZIP (Outline / Looty)">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M17 8l-5-5-5 5M12 3v12" /></svg>
          </button>
          <input type="file" ref="zipInput" accept=".zip" style="display: none;" @change="handleImportZip" />
          <button @click="exportWorkspaceZip" class="icon-btn" title="Exporter tout le wiki en ZIP">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3" /></svg>
          </button>
          <button @click="emit('create-page')" class="icon-btn" title="Créer une page">+</button>
        </div>
      </div>
      
      <div v-if="isProcessingZip" style="padding: 1rem; text-align: center; font-size: 0.85rem; color: var(--text-color-2); background: var(--bg-color-2); border-radius: var(--radius-md); margin: 0 1rem 1rem 1rem;">
        <svg class="spin-icon" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="margin: 0 auto 0.5rem auto; display: block;"><path d="M21 12a9 9 0 11-6.219-8.56"/></svg>
        <div style="font-weight: 500; margin-bottom: 0.25rem;">{{ zipProgress.message }}</div>
        <div v-if="zipProgress.total > 0">{{ zipProgress.current }} / {{ zipProgress.total }}</div>
      </div>
      
      <div v-if="activeWorkspaceId && !isLoadingPages && pages.length === 0" class="empty-item">Aucune page</div>
      <div v-else-if="isLoadingPages" class="loading-item">Chargement...</div>
      <PageTree
        v-if="activeWorkspaceId && pages.length > 0"
        :pages="pages"
        :parent-id="null"
        :level="0"
        :active-page="activePage"
        :current-module="currentModule"
        @select-page="emit('select-page', $event)"
        @delete-page="emit('delete-page', $event)"
        @create-subpage="emit('create-subpage', $event)"
        @move-page="emit('move-page', $event)"
      />
    </div>
    
    <div class="sidebar-footer">
      <div class="user-profile" v-if="user?.user">
        <div class="user-avatar" v-if="!user.user.avatar_url">{{ getInitials(user.user.name) }}</div>
        <img v-else :src="user.user.avatar_url" class="user-avatar" style="object-fit: cover;" />
        <div class="user-info">
          <p class="user-name">{{ user.user.name }}</p>
          <p class="user-email">{{ user.user.email }}</p>
        </div>
      </div>
      <div style="display: flex; gap: 8px; align-items: center; justify-content: space-between;">
        <button class="theme-toggle-btn" @click="toggleTheme" :title="'Thème actuel: ' + colorMode">
          <svg v-if="colorMode === 'light'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" /></svg>
          <svg v-else-if="colorMode === 'dark'" xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" /></svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" /></svg>
        </button>
        <div style="display: flex; gap: 8px;">
          <NuxtLink to="/settings" class="logout-btn" style="text-decoration: none;" title="Paramètres Utilisateur">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" /><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" /></svg>
          </NuxtLink>
          <NuxtLink v-if="user?.user?.is_super_admin" to="/admin" class="logout-btn" style="background: var(--primary-color); color: white; text-decoration: none;">Admin</NuxtLink>
          <a href="/api/auth/logout" class="logout-btn">Déconnexion</a>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 240px;
  background-color: var(--bg-color-2);
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-color);
  z-index: 10;
  transition: width var(--transition-smooth);
}
.sidebar-header {
  padding: 16px 16px 8px;
}
.logo {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--text-color);
  letter-spacing: -0.02em;
  display: flex;
  align-items: center;
  gap: 6px;
}
.logo::before {
  content: '✦';
  color: var(--primary-color);
  font-size: 0.875rem;
}
.sidebar-content {
  flex: 1;
  padding: 0 8px;
  overflow-y: auto;
}
.section-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.6875rem;
  color: var(--text-color-3);
  margin-bottom: 2px;
  margin-top: 20px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 0 8px;
}
.icon-btn {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 2px;
  border-radius: var(--radius-xs);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8125rem;
  transition: all var(--transition-fast);
  line-height: 1;
}
.icon-btn:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.workspace-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.loading-item, .empty-item {
  color: var(--text-color-3);
  font-size: 0.75rem;
  padding: 6px 8px;
}
.workspace-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  color: var(--text-color-2);
  font-size: 0.8125rem;
  font-weight: 400;
  transition: all var(--transition-fast);
  position: relative;
}
.workspace-item:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}
.workspace-item.active {
  background: var(--primary-color-subtle);
  color: var(--primary-color);
  font-weight: 500;
}
.ws-avatar {
  width: 18px;
  height: 18px;
  border-radius: var(--radius-xs);
  background: var(--border-color);
  color: var(--text-color-2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.625rem;
  font-weight: 600;
  flex-shrink: 0;
}
.workspace-item.active .ws-avatar {
  background: var(--primary-color);
  color: #fff;
}
.ws-name {
  font-size: 0.8125rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sidebar-footer {
  padding: 12px;
  border-top: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.user-profile {
  display: flex;
  align-items: center;
  gap: 8px;
}
.user-avatar {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-full);
  background: var(--primary-color-glass);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.6875rem;
  color: var(--primary-color);
  font-weight: 600;
  flex-shrink: 0;
}
.user-info p { margin: 0; }
.user-name { font-weight: 500; font-size: 0.8125rem; color: var(--text-color); }
.user-email { font-size: 0.6875rem; color: var(--text-color-3); }
.logout-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  text-decoration: none;
  color: var(--text-color-3);
  font-size: 0.75rem;
  transition: color var(--transition-fast);
  padding: 4px 8px;
  border-radius: var(--radius-sm);
}
.logout-btn:hover {
  color: var(--danger-color);
  background: var(--danger-glass);
}
.theme-toggle-btn {
  background: none;
  border: none;
  font-size: 1rem;
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
  color: var(--text-color-3);
  display: flex;
  align-items: center;
  justify-content: center;
}
.theme-toggle-btn:hover {
  background: var(--bg-color-hover);
  color: var(--text-color-2);
}
.item-actions {
  display: none;
  gap: 2px;
  position: absolute;
  right: 4px;
  background: var(--bg-color-2);
  padding: 2px;
  border-radius: var(--radius-xs);
  border: 1px solid var(--border-color);
}
.workspace-item:hover .item-actions {
  display: flex;
}
.item-actions button {
  background: none;
  border: none;
  color: var(--text-color-3);
  cursor: pointer;
  padding: 3px;
  border-radius: var(--radius-xs);
  font-size: 0.75rem;
  transition: all var(--transition-fast);
  display: flex;
  align-items: center;
  justify-content: center;
}
.item-actions button:hover {
  background: var(--bg-color-hover);
  color: var(--text-color);
}

/* Spotlight Search */
.spotlight-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  z-index: 1000;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 18vh;
}

.spotlight-modal {
  width: 100%;
  max-width: 560px;
  background: var(--bg-color-modal);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-float);
  border: 1px solid var(--border-color);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: spotlightIn 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes spotlightIn {
  from { opacity: 0; transform: translateY(-8px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.spotlight-input-wrapper {
  display: flex;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid var(--border-color);
}

.spotlight-icon {
  color: var(--text-color-3);
  margin-right: 10px;
  flex-shrink: 0;
}

.spotlight-input {
  flex: 1;
  background: transparent;
  border: none;
  padding: 14px 0;
  font-size: 1rem;
  color: var(--text-color);
  outline: none;
  font-family: inherit;
}
.spotlight-input::placeholder {
  color: var(--text-color-3);
  font-weight: 400;
}

.spotlight-shortcut {
  font-size: 0.6875rem;
  color: var(--text-color-3);
  background: var(--bg-color-3);
  padding: 2px 6px;
  border-radius: var(--radius-xs);
  font-weight: 500;
  border: 1px solid var(--border-color);
  font-family: var(--font-mono);
}

.spotlight-results {
  max-height: 320px;
  overflow-y: auto;
  padding: 8px;
}

.spotlight-state {
  padding: 24px 16px;
  text-align: center;
  color: var(--text-color-3);
  font-size: 0.875rem;
}

.spotlight-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spotlight-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.spotlight-item:hover, .spotlight-item.is-selected {
  background: var(--bg-color-hover);
}

.spotlight-item-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  background: var(--bg-color-3);
  color: var(--text-color-2);
  flex-shrink: 0;
}
.spotlight-item:hover .spotlight-item-icon, .spotlight-item.is-selected .spotlight-item-icon {
  background: var(--primary-color-glass);
  color: var(--primary-color);
}

.spotlight-item-content {
  flex: 1;
  overflow: hidden;
  min-width: 0;
}

.spotlight-item-title {
  font-weight: 500;
  font-size: 0.875rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-item-snippet {
  font-size: 0.75rem;
  color: var(--text-color-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-item-arrow {
  color: var(--text-color-3);
  opacity: 0;
  transform: translateX(-4px);
  transition: all var(--transition-fast);
  flex-shrink: 0;
}
.spotlight-item:hover .spotlight-item-arrow, .spotlight-item.is-selected .spotlight-item-arrow {
  opacity: 1;
  transform: translateX(0);
  color: var(--text-color-2);
}



@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.spin-icon {
  animation: spin 1s linear infinite;
}

</style>
