<script setup lang="ts">
import 'katex/dist/katex.min.css'
import { renderChatMarkdown } from '~/editor/chatMarkdown'
import { enhanceRich } from '~/editor/richMarkdown'
import { diagramStyleSettings, resolveStyle } from '~/editor/diagramStyles'
import { useTheme } from '~/composables/useTheme'
import { useI18n } from '~/composables/useI18n'

/**
 * Markdown riche en lecture : formules LaTeX, diagrammes Mermaid, graphes `looty-graph`,
 * encadrés `:::info`, tâches, tableaux, code coloré. `inline` : sans bloc englobant (choix de
 * réponse) ; `auto` : en ligne tant que le texte tient sur une ligne (énoncé court).
 */
const props = withDefaults(defineProps<{ src: string; mode?: 'block' | 'inline' | 'auto' }>(), { mode: 'block' })
const { t } = useI18n()
const { mode: theme } = useTheme()

const inline = computed(() => props.mode === 'inline' || (props.mode === 'auto' && !props.src.includes('\n')))
const html = computed(() => renderChatMarkdown(props.src ?? '', { inline: inline.value }))
const root = ref<HTMLElement | null>(null)
// Changer de thème ou de style de diagramme reconstruit le contenu (les diagrammes se redessinent).
const epoch = ref(0)
const isDark = () => document.documentElement.classList.contains('dark') || !document.documentElement.classList.contains('light')

function run() {
  if (!root.value) return
  enhanceRich(root.value, { dark: isDark(), style: resolveStyle(null, diagramStyleSettings.value), errorLabel: t('rich.diagramError') })
}
onMounted(run)
watch(html, () => nextTick(run), { flush: 'post' })
watch([theme, diagramStyleSettings], () => { epoch.value++; nextTick(run) })
</script>

<template>
  <!-- eslint-disable vue/no-v-html : Markdown nettoyé par DOMPurify -->
  <span v-if="inline" :key="epoch" ref="root" class="rich rich-inline" v-html="html" />
  <div v-else :key="epoch" ref="root" class="rich" v-html="html" />
</template>

<style scoped>
.rich { min-width: 0; overflow-wrap: anywhere; line-height: 1.55; }
.rich > :deep(:first-child) { margin-top: 0; }
.rich > :deep(:last-child) { margin-bottom: 0; }
.rich :deep(p) { margin: 0 0 8px; }
.rich :deep(ul), .rich :deep(ol) { margin: 0 0 8px; padding-left: 22px; }
.rich :deep(h1), .rich :deep(h2), .rich :deep(h3), .rich :deep(h4), .rich :deep(h5), .rich :deep(h6) { margin: 10px 0 4px; font-size: 1.05em; }
.rich :deep(a) { color: var(--accent); }
.rich :deep(pre) { overflow-x: auto; margin: 0 0 8px; padding: 8px 10px; border-radius: var(--r-sm); background: var(--bg-code, var(--bg-elevated)); font-size: var(--fs-xs); }
.rich :deep(code) { font-family: var(--font-mono); font-size: 0.92em; }
.rich :deep(:not(pre) > code) { padding: 0 4px; border-radius: 3px; background: var(--bg-code, var(--bg-hover)); }
.rich :deep(table) { display: block; max-width: 100%; overflow-x: auto; border-collapse: collapse; margin: 0 0 8px; font-size: var(--fs-sm); }
.rich :deep(th), .rich :deep(td) { border: 1px solid var(--border); padding: 3px 8px; }
.rich :deep(th) { background: var(--bg-hover); text-align: left; }
.rich :deep(blockquote) { margin: 0 0 8px; padding-left: 10px; border-left: 3px solid var(--border-strong, var(--border)); color: var(--text-2); }
.rich :deep(.katex-display) { overflow-x: auto; overflow-y: hidden; margin: 6px 0; }
.rich :deep(.callout) { margin: 0 0 8px; padding: 8px 12px; border-radius: var(--r-md); border-left: 3px solid var(--c-line, var(--border-strong)); background: var(--c-bg); }
.rich :deep(.callout > :last-child) { margin-bottom: 0; }
.rich :deep(.callout-info) { --c-bg: rgba(78, 167, 252, 0.1); --c-line: var(--blue, #4ea7fc); }
.rich :deep(.callout-success) { --c-bg: rgba(76, 183, 130, 0.12); --c-line: var(--green, #4cb782); }
.rich :deep(.callout-warning) { --c-bg: rgba(242, 153, 74, 0.13); --c-line: var(--orange, #f2994a); }
.rich :deep(.callout-tip) { --c-bg: rgba(177, 140, 255, 0.12); --c-line: var(--purple, #b18cff); }
.rich :deep(.rich-check) { display: inline-block; vertical-align: -2px; width: 14px; height: 14px; margin-right: 4px; border: 1.5px solid var(--border-strong, var(--text-3)); border-radius: 3px; position: relative; }
.rich :deep(.rich-check[aria-checked='true']) { background: var(--accent); border-color: var(--accent); }
.rich :deep(.rich-check[aria-checked='true'])::after { content: ''; position: absolute; left: 3.5px; top: 0.5px; width: 4px; height: 8px; border: solid var(--accent-text, #fff); border-width: 0 2px 2px 0; transform: rotate(45deg); }
.rich :deep(li:has(> .rich-check)) { list-style: none; margin-left: -22px; }
.rich :deep(.rich-diagram) { margin: 0 0 8px; }
.rich :deep(.rich-render) { display: flex; justify-content: center; overflow-x: auto; padding: 10px; border-radius: var(--r-md); }
.rich :deep(.rich-render svg) { max-width: 100%; height: auto; }
.rich :deep(.rich-render img) { max-width: 100%; max-height: 420px; }
.rich :deep(.rich-err) { display: block; margin-top: 4px; color: var(--red); font-size: var(--fs-xs); }
</style>
