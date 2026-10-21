# Moodle course personalization is not applied to the Looty calendar

Short name, colour and icon given to a Moodle course (backlog K) are applied in every Moodle screen of Looty (web, desktop, Android) but not to events of the Looty calendar that come from Moodle.

Decision: **not applied.**

Why:
- Those events reach the calendar through Moodle's iCalendar export. The feed carries no course identifier: the only hint is free text (the event title or `CATEGORIES`), which depends on the site's configuration and language. Matching it to a course would colour some events wrongly and others not at all.
- The calendar already has its own per-calendar colour (`calendar-color`), which the member sets for the whole Moodle calendar.
- The Moodle dashboard and the Deadlines panel are fed by the web-service API, which does give the course id, so deadlines there are personalized reliably.

Revisit if the Moodle sync starts importing events through `core_calendar_get_action_events_by_timesort` (it returns the course id) instead of the ICS feed.
