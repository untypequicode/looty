<script setup lang="ts">
import { Sparkles, Monitor, Smartphone, Globe } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useReleases, noteHtml } from '~/composables/useReleases'

const { t, locale } = useI18n()
const { unseen, markSeen } = useReleases()
const open = ref(false)
// Une seule fois par chargement, dès que des notes non vues sont connues.
const shown = ref(false)
watch(unseen, (u) => { if (u.length && !shown.value) { shown.value = true; open.value = true } }, { immediate: true })
watch(open, (o) => { if (!o && shown.value) markSeen() })
const icon = { web: Globe, desktop: Monitor, android: Smartphone }
const lang = computed(() => (locale.value === 'fr' ? 'fr' : 'en'))
const date = (d: string) => new Date(d).toLocaleDateString(locale.value, { day: 'numeric', month: 'long', year: 'numeric' })
</script>

<template>
  <UiDialog v-model:open="open" :title="t('releases.whatsNew')" :width="560">
    <div class="wn">
      <section v-for="r in unseen" :key="r.platform + r.version" class="wn-rel">
        <header class="wn-head">
          <span class="wn-ic"><component :is="icon[r.platform]" :size="14" /></span>
          <h3>{{ r.title[lang] }}</h3>
          <span class="wn-ver">{{ t(`releases.platform.${r.platform}`) }} {{ r.version }} · {{ date(r.date) }}</span>
        </header>
        <ul>
          <!-- eslint-disable-next-line vue/no-v-html -->
          <li v-for="(n, i) in r.notes[lang]" :key="i" v-html="noteHtml(n)" />
        </ul>
      </section>
    </div>
    <template #footer>
      <NuxtLink :to="`/w/${$route.params.ws}/whats-new`" class="wn-all" @click="open = false">{{ t('releases.allReleases') }}</NuxtLink>
      <UiButton variant="primary" :icon="Sparkles" @click="open = false">{{ t('releases.gotIt') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.wn { display: flex; flex-direction: column; gap: 18px; max-height: 60vh; overflow-y: auto; }
.wn-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.wn-head h3 { margin: 0; font-size: 15px; font-weight: 600; }
.wn-ic { width: 24px; height: 24px; border-radius: 6px; display: grid; place-items: center; background: var(--accent-soft); color: var(--accent); flex: none; }
.wn-ver { margin-left: auto; font-size: var(--fs-sm); color: var(--text-3); }
ul { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; color: var(--text-2); font-size: var(--fs-md); line-height: 1.5; }
:deep(code) { font-size: 12px; padding: 1px 5px; border-radius: 4px; background: var(--bg-hover); }
.wn-all { margin-right: auto; font-size: var(--fs-sm); color: var(--text-3); align-self: center; }
.wn-all:hover { color: var(--accent); }
</style>
