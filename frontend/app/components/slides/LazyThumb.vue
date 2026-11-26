<script setup lang="ts">
import type * as Y from 'yjs'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { Slide } from '~/slides/model'
import SlideView from './SlideView.vue'

/**
 * Miniature virtualisée : la diapositive n'est rendue (DOM et SVG) que lorsque sa miniature est proche
 * de la zone visible du rail ; hors de vue, il ne reste qu'un cadre vide de la bonne proportion.
 */
const props = defineProps<{ doc: Y.Doc; slide: Slide; size: { w: number; h: number }; tick: number; margin?: number }>()
const emit = defineEmits<{ (e: 'visible', v: boolean): void }>()
const el = ref<HTMLElement | null>(null)
const shown = ref(false)
let io: IntersectionObserver | null = null
onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') { shown.value = true; return }
  io = new IntersectionObserver((entries) => {
    const v = entries.some((e) => e.isIntersecting)
    if (v !== shown.value) { shown.value = v; emit('visible', v) }
  }, { rootMargin: `${props.margin ?? 240}px 0px` })
  if (el.value) io.observe(el.value)
})
onBeforeUnmount(() => io?.disconnect())
</script>

<template>
  <span ref="el" class="lt" :data-rendered="shown ? '1' : '0'" :style="{ aspectRatio: `${size.w} / ${size.h}` }">
    <SlideView v-if="shown" :doc="doc" :slide="slide" :size="size" :tick="tick" />
  </span>
</template>

<style scoped>
.lt { display: block; width: 100%; background: var(--bg-elevated); }
</style>
