<script setup lang="ts">
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { useSlidesDoc } from '~/composables/useSlidesDoc'
import type { Page } from '~/types'
import PresentShow from '~/components/slides/PresentShow.vue'
import PresenterView from '~/components/slides/PresenterView.vue'

/**
 * Fenêtre dédiée à la présentation d'une page (issue #303) : `?mode=presenter` pour la vue
 * présentateur (second écran), sinon le diaporama plein écran (`?kiosk=8` : boucle automatique,
 * `?follow=1` : suivre la personne qui présente).
 */
definePageMeta({ layout: false })
const route = useRoute()
const { t } = useI18n()
const { user } = useAuth()
const wsId = computed(() => String(route.params.ws))
const pageId = String(route.params.page)
const { doc, provider, synced, denied } = useSlidesDoc(pageId)
const page = ref<Page | null>(null)
const presenterMode = computed(() => route.query.mode === 'presenter')
const kiosk = computed(() => Math.max(0, Number(route.query.kiosk) || 0))
onMounted(async () => {
  try { page.value = await api<Page>(`/api/workspaces/${wsId.value}/pages/${pageId}`, { silent: true }) } catch { denied.value = true }
})
const canEdit = computed(() => page.value?.can_edit !== false && !!page.value)
const PALETTE = ['#7170ff', '#4cb782', '#f2994a', '#eb5757', '#4ea7fc', '#b18cff', '#e06ba8', '#26b5ce']
const me = computed(() => {
  const name = user.value?.name ?? '?'
  let h = 0
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return { name, color: PALETTE[h % PALETTE.length]! }
})
watch([provider, me], () => provider.setAwarenessField('user', me.value), { immediate: true })
useHead({ title: () => `${page.value?.title || t('slides.untitled')} - ${t('present.start')}` })
const back = () => navigateTo(`/w/${wsId.value}/doc/${pageId}`)
</script>

<template>
  <div class="pw">
    <p v-if="denied" class="pw-msg" role="alert">{{ t('public.notFound') }}</p>
    <p v-else-if="!synced || !page" class="pw-msg" role="status">{{ t('present.waiting') }}</p>
    <PresenterView v-else-if="presenterMode" :doc="doc" :page-id="pageId" :provider="provider" :can-share="canEdit" />
    <PresentShow v-else :doc="doc" :page-id="pageId" :provider="provider" :can-share="false" :start-following="route.query.follow === '1'" :kiosk="kiosk" embedded @close="back" />
  </div>
</template>

<style scoped>
.pw { position: fixed; inset: 0; background: #111318; color: #f2f3f5; display: grid; place-items: center; }
.pw-msg { margin: 0; }
</style>
