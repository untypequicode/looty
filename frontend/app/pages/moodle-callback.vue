<script setup lang="ts">
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

/**
 * Retour du portail officiel Moodle dans le navigateur : le gestionnaire de protocole
 * web+looty ouvre cette page avec la clé (?link=web+looty://token=…). La page qui a
 * lancé la connexion (fenêtre parente) connecte le compte ; sinon on le fait ici.
 */
definePageMeta({ layout: false })
const { t } = useI18n()
const route = useRoute()
const error = ref('')
const done = ref(false)

onMounted(async () => {
  const link = typeof route.query.link === 'string' ? route.query.link : ''
  const ws = localStorage.getItem('looty.moodle.pending')
  if (!link) { error.value = t('errors.moodle_invalid_token'); return }
  if (window.opener && !window.opener.closed) {
    window.opener.postMessage({ type: 'looty-moodle-token', link }, location.origin)
    done.value = true
    setTimeout(() => window.close(), 400)
    return
  }
  if (!ws) { error.value = t('moodle.callbackNoWorkspace'); return }
  try {
    await api(`/api/workspaces/${ws}/moodle/connect`, { method: 'POST', body: { token: link }, silent: true })
    localStorage.removeItem('looty.moodle.pending')
    done.value = true
    // La page Moodle restée ouverte (lien avec la fenêtre perdu après le passage par le
    // site Moodle) se met à jour ; la fenêtre de connexion se ferme.
    try { new BroadcastChannel('looty-moodle').postMessage({ type: 'connected', ws }) } catch { /* ancien navigateur */ }
    if (window.name === 'looty-moodle') setTimeout(() => window.close(), 400)
    else navigateTo(`/w/${ws}/moodle`, { replace: true })
  } catch (e) { error.value = errorMessage(e) }
})
</script>

<template>
  <div class="cb">
    <p v-if="error" class="cb-err">{{ error }}</p>
    <p v-else-if="done">{{ t('moodle.callbackDone') }}</p>
    <p v-else><UiSpinner :size="16" /> {{ t('moodle.callbackWorking') }}</p>
  </div>
</template>

<style scoped>
.cb { min-height: 100vh; display: grid; place-items: center; color: var(--text-2); }
.cb p { display: flex; align-items: center; gap: 8px; }
.cb-err { color: var(--red); }
</style>
