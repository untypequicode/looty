<script setup lang="ts">
import { Palette, RotateCcw } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'

/** Choix de la couleur d'un calendrier (pastilles + couleur libre). */
const CAL_COLORS = ['#5e6ad2', '#4cb782', '#e2a03f', '#eb5757', '#26b5ce', '#bb87fc', '#f2994a', '#95a2b3', '#e87ba4', '#2a78d6']

const props = defineProps<{ color: string; resettable?: boolean }>()
const emit = defineEmits<{ pick: [color: string | null] }>()
const { t } = useI18n()
const open = ref(false)
function choose(c: string | null) {
  open.value = false
  if (c !== props.color) emit('pick', c)
}
</script>

<template>
  <UiPopover v-model:open="open" placement="bottom-end" :width="200">
    <template #trigger>
      <UiButton size="xs" variant="ghost" :icon="Palette" class="cs-more" :aria-label="t('calColor.change')" :title="t('calColor.change')" />
    </template>
    <div class="cp">
      <button v-for="c in CAL_COLORS" :key="c" class="sw" :class="{ on: color.toLowerCase() === c }" :style="{ background: c }" :aria-label="c" @click="choose(c)" />
      <label class="sw custom" :title="t('calColor.change')">
        <input type="color" :value="color.startsWith('#') ? color.slice(0, 7) : '#5e6ad2'" @change="choose(($event.target as HTMLInputElement).value)" />
      </label>
      <button v-if="resettable" class="reset" @click="choose(null)"><RotateCcw :size="12" /> {{ t('calColor.reset') }}</button>
    </div>
  </UiPopover>
</template>

<style scoped>
.cp { display: grid; grid-template-columns: repeat(6, 1fr); gap: 6px; padding: 10px; }
.sw { width: 22px; height: 22px; border-radius: 999px; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, .15); cursor: pointer; }
.sw.on { box-shadow: 0 0 0 2px var(--bg-elevated), 0 0 0 4px var(--text-2); }
.custom { position: relative; overflow: hidden; background: conic-gradient(#eb5757, #e2a03f, #4cb782, #26b5ce, #5e6ad2, #bb87fc, #eb5757); }
.custom input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
.reset { grid-column: 1 / -1; display: flex; align-items: center; gap: 6px; height: 26px; padding: 0 6px; border-radius: var(--r-sm); font-size: var(--fs-sm); color: var(--text-2); }
.reset:hover { background: var(--bg-hover); }
</style>
