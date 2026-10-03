<script setup lang="ts">
import { FileQuestion } from 'lucide-vue-next'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

const route = useRoute()
const { t, formatDate } = useI18n()
const page = ref<{ id: string; title: string; icon?: string | null; content?: string | null; updated_at: string } | null>(null)
const missing = ref(false)

onMounted(async () => {
  try {
    page.value = await api(`/api/public/pages/${route.params.token}`, { silent: true })
  } catch {
    missing.value = true
  }
})
useHead({ title: () => page.value?.title || t('docs.untitled') })
</script>

<template>
  <div class="public">
    <UiEmpty v-if="missing" :icon="FileQuestion" :title="t('public.notFound')" :description="t('public.notFoundHint')" />
    <article v-else-if="page" class="doc">
      <div v-if="page.icon" class="icon">{{ page.icon }}</div>
      <h1 class="title">{{ page.title || t('docs.untitled') }}</h1>
      <p class="meta">{{ t('docs.updatedOn', { date: formatDate(page.updated_at) }) }}</p>
      <DocViewer :html="page.content" />
    </article>
    <div v-else class="doc"><div class="skeleton" style="height: 40px; width: 60%" /><div class="skeleton" style="height: 300px; margin-top: 24px" /></div>
  </div>
</template>

<style scoped>
.public { min-height: 100dvh; padding: 64px 24px 96px; }
.doc { max-width: 760px; margin: 0 auto; }
.icon { font-size: 48px; line-height: 1; margin-bottom: 12px; }
.title { font-family: var(--font-display); font-size: 36px; font-weight: 700; letter-spacing: -0.025em; margin: 0; line-height: 1.2; }
.meta { color: var(--text-3); margin: 10px 0 32px; font-size: var(--fs-sm); }
@media (max-width: 639px) { .public { padding: 32px 18px 64px; } .title { font-size: 28px; } }
</style>
