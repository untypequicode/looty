<script setup lang="ts">
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleConversation } from '~/composables/useMoodle'

/** Panneau « Messages récents » : un clic ouvre la conversation dans l'onglet Messages. */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{ (e: 'conversation', id: number): void }>()
const { t, relativeTime } = useI18n()
const convs = useMoodleData<MoodleConversation[]>(toRef(props, 'ws'), 'conversations', ref(true), 60_000)
</script>

<template>
  <div>
    <div v-if="convs.isLoading.value"><div v-for="i in 3" :key="i" class="skeleton" style="height: 44px; margin: 4px 0" /></div>
    <p v-else-if="convs.error.value" class="mp-err">{{ errorMessage(convs.error.value) }}</p>
    <p v-else-if="!convs.data.value?.length" class="mp-empty">{{ t('moodle.noMessages') }}</p>
    <button v-for="c in convs.data.value" :key="c.id" class="cv" :class="{ unread: c.unread > 0 }" @click="emit('conversation', c.id)">
      <UiAvatar :name="c.name" :size="26" />
      <span class="cv-body">
        <span class="cv-top"><b class="truncate">{{ c.name }}</b><time v-if="c.last_time">{{ relativeTime(c.last_time) }}</time></span>
        <span class="cv-last truncate">{{ c.last_from_me ? `${t('moodle.you')} : ` : '' }}{{ c.last_text }}</span>
      </span>
      <span v-if="c.unread" class="cv-badge">{{ c.unread }}</span>
    </button>
  </div>
</template>

<style scoped>
.cv { width: 100%; display: flex; align-items: center; gap: 10px; padding: 6px; border-radius: var(--r-md); text-align: left; }
.cv:hover { background: var(--bg-hover); }
.cv-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.cv-top { display: flex; align-items: center; gap: 6px; }
.cv-top b { flex: 1; min-width: 0; font-weight: 500; }
.cv.unread .cv-top b { font-weight: 700; }
.cv-top time { font-size: var(--fs-xs); color: var(--text-3); flex: none; }
.cv-last { font-size: var(--fs-sm); color: var(--text-3); }
.cv-badge { min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: var(--accent); color: #fff; font-size: 11px; font-weight: 600; display: grid; place-items: center; }
</style>
