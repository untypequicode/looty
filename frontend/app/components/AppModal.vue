<script setup lang="ts">
const props = defineProps<{
  title: string
  submitText?: string
  cancelText?: string
  modelValue?: string
  placeholder?: string
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'submit', value: string): void
  (e: 'update:modelValue', value: string): void
}>()
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal">
      <h2>{{ title }}</h2>
      <form @submit.prevent="emit('submit', modelValue || '')">
        <input 
          v-if="modelValue !== undefined"
          v-focus 
          :value="modelValue"
          @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
          type="text" 
          :placeholder="placeholder" 
          class="modern-input" 
          required
        >
        
        <slot></slot>
        
        <div class="modal-actions">
          <button type="button" class="secondary-btn" @click="emit('close')">{{ cancelText || 'Annuler' }}</button>
          <button type="submit" class="primary-btn" :disabled="disabled">{{ submitText || 'Confirmer' }}</button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(4px); }
.modal { width: 100%; max-width: 420px; padding: 1.25rem; border-radius: 8px; background: var(--bg-color-modal); border: 1px solid var(--border-color); box-shadow: 0 20px 60px rgba(0,0,0,0.6); }
.modal h2 { margin: 0 0 0.75rem 0; font-size: 1rem; font-weight: 500; }
.modern-input { width: 100%; padding: 6px 10px; border-radius: 6px; background: #111; border: 1px solid var(--border-color); color: #eee; font-family: inherit; font-size: 0.85rem; margin-bottom: 0.75rem; box-sizing: border-box; }
.modern-input:focus { border-color: var(--primary-color); outline: none; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1rem; }
.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: var(--primary-color-hover); }
.primary-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.secondary-btn { padding: 6px 14px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.secondary-btn:hover { background: var(--border-color); color: #eee; }
</style>
