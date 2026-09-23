<script setup lang="ts">
import type { Board, BoardAutomation } from '../types'

const props = defineProps<{
  activeBoard: Board | null
  boardSettings: BoardAutomation
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', settings: BoardAutomation): void
  (e: 'update:boardSettings', settings: BoardAutomation): void
}>()

const updateSetting = (key: keyof BoardAutomation, value: any) => {
  emit('update:boardSettings', { ...props.boardSettings, [key]: value })
}
</script>

<template>
  <div class="modal-overlay" @click.self="emit('close')">
    <div class="modal">
      <h2>Paramètres du Tableau</h2>
      <form @submit.prevent="emit('save', boardSettings)">
        
        <div class="settings-section">
          <h3>Automatisations</h3>
          
          <label class="setting-toggle">
            <input 
              type="checkbox" 
              :checked="!!boardSettings.on_complete_move_to" 
              @change="(e) => updateSetting('on_complete_move_to', (e.target as HTMLInputElement).checked ? activeBoard?.columns[0]?.id || '' : '')"
            >
            Déplacer automatiquement les tâches terminées
          </label>
          
          <div v-if="boardSettings.on_complete_move_to" class="sub-settings">
            <div class="select-container">
              <label>Colonne de destination :</label>
              <select 
                :value="boardSettings.on_complete_move_to"
                @change="(e) => updateSetting('on_complete_move_to', (e.target as HTMLSelectElement).value)"
                class="modern-input"
              >
                <option v-for="col in activeBoard?.columns" :key="col.id" :value="col.id">{{ col.name }}</option>
              </select>
            </div>
            
            <label class="setting-toggle small">
              <input 
                type="checkbox" 
                :checked="boardSettings.apply_to_subtasks"
                @change="(e) => updateSetting('apply_to_subtasks', (e.target as HTMLInputElement).checked)"
              >
              Appliquer également aux sous-tâches
            </label>

            <label class="setting-toggle small">
              <input 
                type="checkbox" 
                :checked="boardSettings.auto_validate_on_move"
                @change="(e) => updateSetting('auto_validate_on_move', (e.target as HTMLInputElement).checked)"
              >
              Valider / Décocher automatiquement lors du déplacement manuel
            </label>
          </div>
        </div>

        <div class="modal-actions">
          <button type="button" class="secondary-btn" @click="emit('close')">Annuler</button>
          <button type="submit" class="primary-btn">Enregistrer</button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.7); display: flex; align-items: center; justify-content: center; z-index: 100; backdrop-filter: blur(4px); }
.modal { width: 100%; max-width: 420px; padding: 1.25rem; border-radius: 8px; background: var(--bg-color-modal); border: 1px solid var(--border-color); box-shadow: 0 20px 60px rgba(0,0,0,0.6); }
.modal h2 { margin: 0 0 0.75rem 0; font-size: 1rem; font-weight: 500; }
.settings-section { margin-top: 1.5rem; background: var(--bg-color); padding: 1rem; border-radius: 8px; border: 1px solid var(--border-color); }
.settings-section h3 { margin-top: 0; font-size: 1rem; color: #eee; margin-bottom: 1rem; }
.setting-toggle { display: flex; align-items: center; gap: 8px; cursor: pointer; color: #d1d5db; font-size: 0.9rem; margin-bottom: 1rem; }
.setting-toggle.small { font-size: 0.85rem; margin-bottom: 0.5rem; }
.sub-settings { margin-left: 24px; display: flex; flex-direction: column; gap: 1rem; }
.select-container label { display: block; font-size: 0.8rem; color: var(--text-color-3); margin-bottom: 6px; }
.modern-input { width: 100%; padding: 6px 10px; border-radius: 6px; background: #111; border: 1px solid var(--border-color); color: #eee; font-family: inherit; font-size: 0.85rem; margin-bottom: 0; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 1.5rem; }
.primary-btn { padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer; background: var(--primary-color); color: #fff; font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.primary-btn:hover { background: var(--primary-color-hover); }
.secondary-btn { padding: 6px 14px; border-radius: 6px; border: 1px solid var(--border-color); cursor: pointer; background: transparent; color: var(--text-color-3); font-weight: 500; font-size: 0.85rem; transition: all 0.12s; }
.secondary-btn:hover { background: var(--border-color); color: #eee; }
</style>
