<script setup lang="ts">
import { CheckCircle2, AlertCircle, Info, X } from 'lucide-vue-next'
import gsap from 'gsap'
import { useToasts } from '~/composables/useToast'

const { toasts, dismissToast } = useToasts()
const icons = { success: CheckCircle2, error: AlertCircle, info: Info }

function onEnter(el: Element, done: () => void) {
  gsap.fromTo(el, { opacity: 0, y: 16, scale: 0.96 }, { opacity: 1, y: 0, scale: 1, duration: 0.3, ease: 'power3.out', onComplete: done })
}
function onLeave(el: Element, done: () => void) {
  gsap.to(el, { opacity: 0, x: 24, duration: 0.18, ease: 'power2.in', onComplete: done })
}
</script>

<template>
  <Teleport to="body">
    <TransitionGroup tag="div" class="toaster" :css="false" @enter="onEnter" @leave="onLeave">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="t.kind" role="status">
        <component :is="icons[t.kind]" :size="16" class="toast-icon" />
        <span class="toast-msg">{{ t.message }}</span>
        <button v-if="t.action" class="toast-action" @click="t.action.run(); dismissToast(t.id)">{{ t.action.label }}</button>
        <button class="toast-close" :aria-label="$t('common.close')" @click="dismissToast(t.id)"><X :size="14" /></button>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<style scoped>
.toaster {
  position: fixed;
  right: 16px;
  bottom: calc(16px + var(--safe-bottom));
  z-index: 2000;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: flex-end;
  pointer-events: none;
}
.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 260px;
  max-width: 420px;
  padding: 10px 10px 10px 12px;
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
  font-size: var(--fs-md);
}
.toast-icon { flex: none; color: var(--text-2); }
.toast.success .toast-icon { color: var(--green); }
.toast.error .toast-icon { color: var(--red); }
.toast-msg { flex: 1; line-height: 1.4; }
.toast-action { color: var(--accent-text); font-weight: 500; padding: 2px 6px; border-radius: var(--r-xs); }
.toast-action:hover { background: var(--accent-soft); }
.toast-close { color: var(--text-3); display: grid; place-items: center; width: 22px; height: 22px; border-radius: var(--r-xs); }
.toast-close:hover { background: var(--bg-hover); color: var(--text); }
@media (max-width: 639px) {
  .toaster { left: 12px; right: 12px; align-items: stretch; bottom: calc(72px + var(--safe-bottom)); }
  .toast { max-width: none; }
}
</style>
