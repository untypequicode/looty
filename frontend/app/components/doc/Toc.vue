<script setup lang="ts">
import type { HeadingInfo } from '~/editor/headings'
import { motion } from '~/composables/useMotion'

/** Sommaire du document (façon Outline) avec suivi du titre courant. */
const props = defineProps<{ headings: HeadingInfo[]; scroller: HTMLElement | null }>()
const activeId = ref<string | null>(null)
const minLevel = computed(() => Math.min(...props.headings.map((h) => h.level), 4))

function update() {
  const sc = props.scroller
  if (!sc) return
  const top = sc.getBoundingClientRect().top + 90
  let current: string | null = null
  for (const h of props.headings) {
    const el = document.getElementById(h.id)
    if (!el) continue
    if (el.getBoundingClientRect().top <= top) current = h.id
    else break
  }
  activeId.value = current ?? props.headings[0]?.id ?? null
}

watch(() => props.scroller, (sc, old) => {
  old?.removeEventListener('scroll', update)
  sc?.addEventListener('scroll', update, { passive: true })
  update()
}, { immediate: true })
watch(() => props.headings, () => nextTick(update))
onBeforeUnmount(() => props.scroller?.removeEventListener('scroll', update))

function go(h: HeadingInfo) {
  const el = document.getElementById(h.id)
  if (!el || !props.scroller) return
  const y = el.getBoundingClientRect().top - props.scroller.getBoundingClientRect().top + props.scroller.scrollTop - 70
  props.scroller.scrollTo({ top: y, behavior: 'smooth' })
  history.replaceState(null, '', `#${h.id}`)
  setTimeout(() => motion.flash(el), 300)
}
</script>

<template>
  <nav v-if="headings.length" class="toc" :aria-label="$t('editor.contents')">
    <div class="toc-title">{{ $t('editor.contents') }}</div>
    <a
      v-for="h in headings"
      :key="h.id + h.pos"
      :href="`#${h.id}`"
      class="toc-item"
      :class="{ active: activeId === h.id }"
      :style="{ paddingLeft: `${(h.level - minLevel) * 12 + 10}px` }"
      @click.prevent="go(h)"
    >{{ h.text || '…' }}</a>
  </nav>
</template>

<style scoped>
.toc { display: flex; flex-direction: column; gap: 1px; }
.toc-title { font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); padding: 0 10px 8px; text-transform: uppercase; letter-spacing: 0.04em; }
.toc-item {
  position: relative;
  padding: 4px 10px;
  font-size: var(--fs-sm);
  color: var(--text-3);
  border-radius: var(--r-sm);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: color var(--dur-fast), background var(--dur-fast);
}
.toc-item:hover { color: var(--text); background: var(--bg-hover); }
.toc-item.active { color: var(--accent-text); }
.toc-item.active::before { content: ''; position: absolute; left: 0; top: 6px; bottom: 6px; width: 2px; border-radius: 2px; background: var(--accent); }
</style>
