// node --test frontend/tests/reminders.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { REMINDER_PRESETS, reminderLabel } from '../app/utils/reminders.ts'

const t = (k: string, p?: Record<string, unknown>) => `${k}${p ? JSON.stringify(p) : ''}`

test('minutes are described with the largest whole unit', () => {
  assert.equal(reminderLabel(null, t), 'taskReminder.none')
  assert.equal(reminderLabel(0, t), 'taskReminder.atDue')
  assert.equal(reminderLabel(45, t), 'taskReminder.minutesBefore{"count":45}')
  assert.equal(reminderLabel(60, t), 'taskReminder.hoursBefore{"count":1}')
  assert.equal(reminderLabel(2880, t), 'taskReminder.daysBefore{"count":2}')
  assert.equal(reminderLabel(10080, t), 'taskReminder.weeksBefore{"count":1}')
})

test('presets are sorted and stay within the 30 days the server accepts', () => {
  assert.deepEqual([...REMINDER_PRESETS], [...REMINDER_PRESETS].sort((a, b) => a - b))
  assert.ok(REMINDER_PRESETS.every((m) => m >= 0 && m <= 43200))
})
