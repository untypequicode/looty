<script setup lang="ts">
import { Sigma, Package, AlertTriangle } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import { api } from '~/composables/api'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { scopeBuilder, renderMath, ensureMhchem } from '~/editor/latex'

const { wsId, isAdmin } = useWorkspace()
const { t } = useI18n()
const qc = useQueryClient()
const { data } = useQuery({ queryKey: computed(() => wsKeys.latex(wsId.value)), queryFn: () => api<{ preamble: string }>(`/api/workspaces/${wsId.value}/latex`) })
const text = ref('')
watch(() => data.value?.preamble, (p) => { text.value = p ?? '' }, { immediate: true })
const dirty = computed(() => text.value !== (data.value?.preamble ?? ''))

// Analyse en direct : noms définis, paquets, erreurs, aperçu.
const tick = ref(0)
const analysis = computed(() => { void tick.value; return scopeBuilder().next(text.value) })
watch(() => analysis.value.needsMhchem, (m) => { if (m) ensureMhchem(() => tick.value++) }, { immediate: true })
const sample = ref('')
const preview = computed(() => {
  void tick.value
  return sample.value.trim() ? renderMath(katex, sample.value, scopeBuilder(analysis.value.macros, analysis.value.needsMhchem).next(''), true) : null
})

const saving = ref(false)
async function save() {
  saving.value = true
  try {
    const r = await api<{ preamble: string }>(`/api/workspaces/${wsId.value}/latex`, { method: 'PUT', body: { preamble: text.value } })
    qc.setQueryData(wsKeys.latex(wsId.value), r)
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
const placeholder = '\\usepackage{physics}\n\\newcommand{\\R}{\\mathbb{R}}\n\\newcommand{\\eq}{\\Leftrightarrow}\n\\DeclareMathOperator{\\Var}{Var}'
</script>

<template>
  <SettingsSection :title="t('settings.latex.title')" :description="t('settings.latex.description')">
    <div class="lp">
      <textarea v-model="text" class="lp-text" :readonly="!isAdmin" spellcheck="false" :placeholder="placeholder" rows="8" />
      <div v-if="analysis.parsed.error" class="lp-note warn"><AlertTriangle :size="13" /> {{ analysis.parsed.error }}</div>
      <div v-if="analysis.parsed.rest" class="lp-note warn"><AlertTriangle :size="13" /> {{ t('settings.latex.ignored') }}</div>
      <div v-if="analysis.defines.length || analysis.packages.length" class="lp-chips">
        <Sigma :size="13" class="faint" />
        <code v-for="d in analysis.defines" :key="d">{{ d }}</code>
        <span v-for="p in analysis.packages" :key="p.name" class="lp-pkg" :class="p.status" :title="p.status === 'unknown' ? t('editor.mathPackageUnknown', { name: p.name }) : ''"><Package :size="11" /> {{ p.name }}</span>
      </div>
      <div class="lp-try">
        <input v-model="sample" :placeholder="t('settings.latex.tryPlaceholder')" spellcheck="false" />
        <div v-if="preview" class="lp-preview">
          <div v-if="preview.html" v-html="preview.html" />
          <span v-else class="lp-err">{{ preview.error }}</span>
        </div>
      </div>
      <div class="lp-foot">
        <span class="faint">{{ isAdmin ? t('settings.latex.hint') : t('settings.latex.adminOnly') }}</span>
        <UiButton v-if="isAdmin" size="sm" variant="primary" :disabled="!dirty" :loading="saving" @click="save">{{ t('common.save') }}</UiButton>
      </div>
    </div>
  </SettingsSection>
</template>

<style scoped>
.lp { padding: 14px 16px; display: flex; flex-direction: column; gap: 10px; }
.lp-text { width: 100%; min-height: 150px; resize: vertical; padding: 10px 12px; border-radius: var(--r-md); border: 1px solid var(--border-strong); background: var(--bg-input); font-family: var(--font-mono); font-size: 12.5px; line-height: 1.55; color: var(--text); outline: none; }
.lp-text:focus { border-color: var(--border-focus); }
.lp-note { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-3); }
.lp-note.warn { color: var(--orange, #e2a03f); }
.lp-chips { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.lp-chips code { font-family: var(--font-mono); font-size: 12px; padding: 1px 5px; border-radius: 3px; background: var(--bg-code); color: var(--text-2); }
.lp-pkg { display: inline-flex; align-items: center; gap: 3px; font-size: 12px; color: var(--text-3); }
.lp-pkg.unknown { color: var(--orange, #e2a03f); }
.lp-try { display: flex; flex-direction: column; gap: 6px; }
.lp-try input { height: 32px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-family: var(--font-mono); font-size: 12.5px; outline: none; color: var(--text); }
.lp-preview { padding: 8px; border-radius: var(--r-md); background: var(--bg-code); overflow-x: auto; }
.lp-err { color: var(--red); font-family: var(--font-mono); font-size: var(--fs-sm); }
.lp-foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-size: var(--fs-sm); }
</style>
