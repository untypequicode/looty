/** Rappels proposés pour une tâche (minutes avant l'échéance). */
export const REMINDER_PRESETS = [0, 60, 1440, 2880, 10080]

/** « À l'échéance », « 1 h avant », « 2 jours avant »… ; `null` : aucun. */
export function reminderLabel(minutes: number | null, t: (k: string, p?: Record<string, unknown>) => string): string {
  if (minutes == null) return t('taskReminder.none')
  if (minutes === 0) return t('taskReminder.atDue')
  if (minutes % 10080 === 0) return t('taskReminder.weeksBefore', { count: minutes / 10080 })
  if (minutes % 1440 === 0) return t('taskReminder.daysBefore', { count: minutes / 1440 })
  if (minutes % 60 === 0) return t('taskReminder.hoursBefore', { count: minutes / 60 })
  return t('taskReminder.minutesBefore', { count: minutes })
}
