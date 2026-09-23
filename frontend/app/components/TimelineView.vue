<template>
  <div class="timeline-container">
    <!-- Entête des dates -->
    <div class="timeline-header">
      <div class="timeline-left-panel">
        <h3>Tâches</h3>
      </div>
      <div class="timeline-grid-header">
        <div v-for="day in days" :key="day.getTime()" class="timeline-day-col">
          <div class="day-name">{{ getDayName(day) }}</div>
          <div class="day-number" :class="{ 'is-today': isToday(day) }">{{ day.getDate() }}</div>
        </div>
      </div>
    </div>
    
    <!-- Corps de la Timeline -->
    <div class="timeline-body">
      <div v-for="col in board.columns" :key="col.id" class="timeline-column-group">
        <div class="timeline-column-title">{{ col.name }}</div>
        
        <template v-for="task in getTasksWithDates(col.tasks)" :key="task.id">
          <div class="timeline-row">
            <div class="timeline-left-panel">
              <span class="task-title" @click="openTaskDetail(task)">{{ task.title }}</span>
            </div>
            
            <div class="timeline-grid-row">
              <div v-for="day in days" :key="day.getTime()" class="timeline-day-cell"></div>
              
              <div class="timeline-task-bar" 
                   :style="getTaskBarStyle(task)" 
                   :class="{ 'is-completed': task.is_completed }"
                   @click="openTaskDetail(task)">
                <span class="bar-label">{{ task.title }}</span>
              </div>
            </div>
          </div>
        </template>
        
        <!-- Si aucune tâche avec date dans la colonne -->
        <div v-if="getTasksWithDates(col.tasks).length === 0" class="timeline-empty-col">
          <div class="timeline-left-panel"></div>
          <div class="timeline-grid-row">
            <div v-for="day in days" :key="day.getTime()" class="timeline-day-cell"></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  board: any,
  openTaskDetail: (task: any) => void
}>()

const DAY_WIDTH = 48 // pixels

function getTasksWithDates(tasks: any[]) {
  if (!tasks) return []
  return tasks.filter(t => t.start_date || t.due_date)
}

function getAllTasksWithDates() {
  const all: any[] = []
  if (!props.board || !props.board.columns) return all
  for (const col of props.board.columns) {
    if (col.tasks) {
      all.push(...getTasksWithDates(col.tasks))
    }
  }
  return all
}

const dateRange = computed(() => {
  const tasks = getAllTasksWithDates()
  if (tasks.length === 0) {
    const today = new Date()
    const start = new Date(today)
    start.setDate(today.getDate() - 10)
    const end = new Date(today)
    end.setDate(today.getDate() + 20)
    return { start, end }
  }

  let min = new Date(tasks[0].start_date || tasks[0].due_date).getTime()
  let max = new Date(tasks[0].due_date || tasks[0].start_date).getTime()

  for (const t of tasks) {
    const s = t.start_date ? new Date(t.start_date).getTime() : new Date(t.due_date).getTime()
    const e = t.due_date ? new Date(t.due_date).getTime() : new Date(t.start_date).getTime()
    if (s < min) min = s
    if (e > max) max = e
  }

  const start = new Date(min)
  start.setDate(start.getDate() - 3)
  const end = new Date(max)
  end.setDate(end.getDate() + 3)
  return { start, end }
})

const days = computed(() => {
  const arr = []
  const current = new Date(dateRange.value.start)
  const end = dateRange.value.end
  
  current.setHours(0, 0, 0, 0)
  end.setHours(0, 0, 0, 0)

  while (current <= end) {
    arr.push(new Date(current))
    current.setDate(current.getDate() + 1)
  }
  return arr
})

function getDayName(d: Date) {
  const names = ['Dim', 'Lun', 'Mar', 'Mer', 'Jeu', 'Ven', 'Sam']
  return names[d.getDay()]
}

function isToday(d: Date) {
  const today = new Date()
  return d.getDate() === today.getDate() && d.getMonth() === today.getMonth() && d.getFullYear() === today.getFullYear()
}

function getTaskBarStyle(task: any) {
  if (!days.value || days.value.length === 0) return {}

  const timelineStart = days.value[0].getTime()
  
  const start = task.start_date ? new Date(task.start_date) : new Date(task.due_date)
  const end = task.due_date ? new Date(task.due_date) : new Date(task.start_date)
  
  start.setHours(0, 0, 0, 0)
  end.setHours(0, 0, 0, 0)
  
  const diffDaysFromStart = (start.getTime() - timelineStart) / (1000 * 60 * 60 * 24)
  const durationDays = (end.getTime() - start.getTime()) / (1000 * 60 * 60 * 24) + 1 // Inclusive
  
  let color = '#3b82f6' // Default blue
  if (task.labels && task.labels.length > 0) {
    color = task.labels[0].color
  }

  return {
    left: `${diffDaysFromStart * DAY_WIDTH}px`,
    width: `${durationDays * DAY_WIDTH - 4}px`, // -4 for margins
    backgroundColor: color
  }
}
</script>

<style scoped>
.timeline-container {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 120px);
  overflow: hidden;
  background: #0e0e11;
  border-top: 1px solid #27282b;
  margin-top: 1rem;
}

.timeline-header {
  display: flex;
  border-bottom: 1px solid #27282b;
  background: #131418;
}

.timeline-left-panel {
  width: 250px;
  flex-shrink: 0;
  border-right: 1px solid #27282b;
  padding: 12px 16px;
  display: flex;
  align-items: center;
  background: #17181c;
  z-index: 10;
  position: sticky;
  left: 0;
}

.timeline-header .timeline-left-panel h3 {
  margin: 0;
  font-size: 0.9rem;
  color: #8a8f98;
  font-weight: 600;
  text-transform: uppercase;
}

.timeline-grid-header {
  display: flex;
  flex: 1;
  overflow-x: hidden; /* Sync with body scroll */
}

.timeline-day-col {
  width: 48px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8px 0;
  border-right: 1px solid #27282b;
}

.day-name {
  font-size: 0.65rem;
  color: #8a8f98;
  text-transform: uppercase;
}
.day-number {
  font-size: 0.9rem;
  font-weight: 600;
  color: #eee;
  margin-top: 2px;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
}
.day-number.is-today {
  background: #5e6ad2;
  color: #fff;
}

.timeline-body {
  flex: 1;
  overflow: auto;
  position: relative;
}

.timeline-column-group {
  margin-bottom: 0;
}

.timeline-column-title {
  padding: 8px 16px;
  background: #1a1b1f;
  font-weight: 600;
  font-size: 0.85rem;
  color: #9ca3af;
  border-bottom: 1px solid #27282b;
  position: sticky;
  left: 0;
  z-index: 5;
}

.timeline-row {
  display: flex;
  border-bottom: 1px solid #1c1d21;
}
.timeline-row:hover {
  background: #1c1d21;
}

.task-title {
  font-size: 0.85rem;
  color: #d1d5db;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
}
.task-title:hover {
  color: #fff;
}

.timeline-grid-row {
  display: flex;
  position: relative;
}

.timeline-day-cell {
  width: 48px;
  height: 40px;
  flex-shrink: 0;
  border-right: 1px solid #1c1d21;
}

.timeline-task-bar {
  position: absolute;
  top: 6px;
  height: 28px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  padding: 0 8px;
  cursor: pointer;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
  transition: opacity 0.2s, filter 0.2s;
  overflow: hidden;
  z-index: 1;
}
.timeline-task-bar:hover {
  filter: brightness(1.2);
  z-index: 2;
}
.timeline-task-bar.is-completed {
  opacity: 0.5;
  background: #10b981 !important;
}
.timeline-task-bar.is-completed .bar-label {
  text-decoration: line-through;
}

.bar-label {
  color: #fff;
  font-size: 0.75rem;
  font-weight: 500;
  white-space: nowrap;
  text-overflow: ellipsis;
  overflow: hidden;
  text-shadow: 0 1px 2px rgba(0,0,0,0.5);
}

.timeline-empty-col {
  display: flex;
  height: 20px;
  border-bottom: 1px solid #1c1d21;
}
</style>
