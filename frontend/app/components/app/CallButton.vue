<script setup lang="ts">
import { Phone, PhoneOff } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useCallPresence, joinCall, leave, isInRoom, callState } from '~/composables/useCall'
import type { CallKind } from '~/utils/callRtc'

/**
 * Bouton « Rejoindre l'appel » d'une page, d'un projet ou d'un espace, avec la pastille de
 * ceux qui sont déjà dans l'appel. L'appel continue quand on change de page (barre d'appel).
 */
const props = defineProps<{ kind: CallKind; id: string; label: string }>()
const { t } = useI18n()
// Réglage d'instance (admin) : appels désactivés, le bouton disparaît.
const { data: settings } = useQuery({ queryKey: ['public-settings'], queryFn: () => api<{ calls_enabled?: boolean }>('/api/settings', { silent: true }), staleTime: 10 * 60_000 })
const disabled = computed(() => settings.value?.calls_enabled === false)
const route = useRoute()
const { peers, denied } = useCallPresence(() => props.kind, () => props.id)
const here = computed(() => isInRoom(props.kind, props.id))
const busy = computed(() => callState.status === 'joining')
const count = computed(() => peers.value.length)
const names = computed(() => peers.value.map((p) => p.name).join(', '))
const toggle = () => (here.value ? leave() : joinCall(props.kind, props.id, props.label, route.fullPath))
</script>

<template>
  <UiTooltip v-if="!denied && !disabled" :text="here ? t('calls.leave') : names || t('calls.joinHint')">
    <UiButton class="call-btn" size="sm" :variant="here ? 'danger' : count ? 'primary' : 'ghost'" :icon="here ? PhoneOff : Phone" :loading="busy && !here" :aria-label="here ? t('calls.leave') : count ? t('calls.join') : t('calls.start')" @click="toggle">
      <template v-if="here">{{ t('calls.leave') }}</template>
      <template v-else-if="count">{{ t('calls.join') }} · {{ count }}</template>
    </UiButton>
  </UiTooltip>
</template>
