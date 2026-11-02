<script setup lang="ts">
import { Vote, Lock, EyeOff } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import type { Poll } from '~/composables/useAnnouncements'

/** Sondage d'une annonce : voter, changer ou retirer son vote, voir les résultats. */
const props = defineProps<{ poll: Poll; canManage: boolean }>()
const emit = defineEmits<{ (e: 'vote', options: string[]): void; (e: 'close'): void }>()
const { t, formatDateTime } = useI18n()
const { member } = useWorkspace()

const picked = ref<string[]>([...props.poll.my_votes])
watch(() => props.poll.my_votes, (v) => { picked.value = [...v] })
const voted = computed(() => props.poll.my_votes.length > 0)
const dirty = computed(() => [...picked.value].sort().join() !== [...props.poll.my_votes].sort().join())
const total = computed(() => props.poll.options.reduce((n, o) => n + (o.votes ?? 0), 0))
function toggle(id: string) {
  if (props.poll.closed) return
  if (props.poll.multiple) picked.value = picked.value.includes(id) ? picked.value.filter((x) => x !== id) : [...picked.value, id]
  else picked.value = picked.value[0] === id ? [] : [id]
}
const pct = (n: number | null) => (total.value && n ? Math.round((n / total.value) * 100) : 0)
const names = (ids: string[] | null) => (ids ?? []).map((id) => member(id)?.name ?? '?').join(', ')
</script>

<template>
  <div class="poll" :class="{ closed: poll.closed }">
    <div class="poll-q"><Vote :size="15" /> <b>{{ poll.question }}</b></div>
    <div class="poll-hint">
      {{ poll.multiple ? t('polls.multipleHint') : t('polls.singleHint') }}
      <template v-if="poll.anonymous"> · <EyeOff :size="11" /> {{ t('polls.anonymous') }}</template>
      <template v-if="poll.closed"> · <Lock :size="11" /> {{ t('polls.closed') }}</template>
      <template v-else-if="poll.closes_at"> · {{ t('polls.closesAt', { date: formatDateTime(poll.closes_at) }) }}</template>
    </div>
    <div class="poll-opts" role="group" :aria-label="poll.question">
      <button
        v-for="o in poll.options" :key="o.id" class="poll-opt" :class="{ on: picked.includes(o.id) }" :disabled="poll.closed"
        :role="poll.multiple ? 'checkbox' : 'radio'" :aria-checked="picked.includes(o.id)" :title="poll.results_visible && !poll.anonymous ? names(o.voters) : undefined"
        @click="toggle(o.id)"
      >
        <span v-if="poll.results_visible" class="poll-bar" :style="{ width: `${pct(o.votes)}%` }" />
        <span class="poll-mark" :class="poll.multiple ? 'box' : 'dot'" />
        <span class="poll-label">{{ o.label }}</span>
        <span v-if="poll.results_visible" class="poll-count">{{ o.votes }} · {{ pct(o.votes) }} %</span>
      </button>
    </div>
    <div class="poll-foot">
      <span class="poll-total">{{ t('polls.voters', { count: poll.voter_count }) }}<template v-if="!poll.results_visible"> · {{ t('polls.resultsAfterVote') }}</template></span>
      <template v-if="!poll.closed">
        <UiButton v-if="voted && !picked.length" size="sm" variant="ghost" @click="emit('vote', [])">{{ t('polls.retract') }}</UiButton>
        <UiButton v-else size="sm" variant="primary" :disabled="!dirty || !picked.length" @click="emit('vote', picked)">{{ voted ? t('polls.change') : t('polls.vote') }}</UiButton>
        <UiButton v-if="canManage" size="sm" variant="ghost" :icon="Lock" @click="emit('close')">{{ t('polls.close') }}</UiButton>
      </template>
    </div>
  </div>
</template>

<style scoped>
.poll { margin-top: 12px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--bg); }
.poll-q { display: flex; align-items: center; gap: 6px; }
.poll-hint { font-size: var(--fs-xs, 11px); color: var(--text-3); margin: 2px 0 8px; display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.poll-opts { display: flex; flex-direction: column; gap: 6px; }
.poll-opt { position: relative; overflow: hidden; display: flex; align-items: center; gap: 8px; padding: 7px 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); text-align: left; font-size: var(--fs-sm); }
.poll-opt:not(:disabled):hover { background: var(--bg-hover); }
.poll-opt.on { border-color: var(--accent); }
.poll-bar { position: absolute; inset: 0 auto 0 0; background: color-mix(in srgb, var(--accent) 14%, transparent); transition: width var(--dur-fast); }
.poll-mark { position: relative; width: 14px; height: 14px; border: 1.5px solid var(--text-3); flex: none; }
.poll-mark.dot { border-radius: 50%; }
.poll-mark.box { border-radius: 3px; }
.poll-opt.on .poll-mark { border-color: var(--accent); background: var(--accent); box-shadow: inset 0 0 0 2px var(--bg); }
.poll-label { position: relative; flex: 1; min-width: 0; }
.poll-count { position: relative; color: var(--text-2); font-variant-numeric: tabular-nums; }
.poll-foot { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
.poll-total { flex: 1; font-size: var(--fs-sm); color: var(--text-3); }
</style>
