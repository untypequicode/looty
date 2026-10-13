package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable data class User(val id: String, val name: String = "", val email: String = "", val avatar_url: String? = null)
@Serializable data class Me(val authenticated: Boolean = false, val user: User? = null)
@Serializable data class Workspace(val id: String, val name: String, val role: String = "member")
@Serializable data class Workspaces(val workspaces: List<Workspace> = emptyList())
@Serializable data class Member(val user_id: String, val name: String = "", val email: String = "", val avatar_url: String? = null, val role: String = "")
/** Droits du rôle dans l’espace (GET access/me) : none < read < comment < validate < edit < manage. */
@Serializable data class Permissions(val docs: String = "edit", val boards: String = "edit", val calendar: String = "edit", val invite: Boolean = false, val share: Boolean = false, val admin: Boolean = false)
@Serializable data class MyAccess(val role: String = "member", val permissions: Permissions = Permissions())
@Serializable data class Members(val members: List<Member> = emptyList())

@Serializable data class MyCounts(val open: Int = 0, val overdue: Int = 0, val due_week: Int = 0, val completed_week: Int = 0)
@Serializable data class BoardStats(val id: String, val name: String, val total: Int = 0, val done: Int = 0, val overdue: Int = 0, val due_soon: Int = 0)
@Serializable data class Overview(val boards: List<BoardStats> = emptyList(), val my: MyCounts = MyCounts())

@Serializable data class MyTask(
    val id: String,
    val title: String,
    val due_date: String? = null,
    val priority: Int = 0,
    val is_completed: Boolean = false,
    val board_id: String,
    val board_name: String = "",
    val column_name: String = "",
    val is_per_person: Boolean = false,
    val assigned_to_me: Boolean = true,
    val done_by_me: Boolean = false,
    val assignees: List<String> = emptyList(),
)
@Serializable data class MyTasks(val tasks: List<MyTask> = emptyList())

@Serializable data class Page(
    val id: String,
    val parent_id: String? = null,
    val icon: String? = null,
    val title: String = "",
    val position: Double = 0.0,
    val is_favorite: Boolean? = null,
    val updated_at: String? = null,
    val last_viewed_at: String? = null,
    val can_edit: Boolean? = null,
)

@Serializable data class Notification(
    val id: String,
    val workspace_id: String,
    val actor_id: String? = null,
    val actor_name: String? = null,
    val kind: String,
    val target: String,
    val data: JsonObject = JsonObject(emptyMap()),
    val read_at: String? = null,
    val created_at: String,
)

@Serializable data class BoardSummary(val id: String, val name: String)
@Serializable data class Column(val id: String, val name: String, val position: Double = 0.0)
@Serializable data class Board(val id: String, val name: String, val columns: List<Column> = emptyList())

@Serializable data class CalEvent(
    val id: String,
    val calendar_id: String = "",
    val summary: String = "",
    val description: String? = null,
    val location: String? = null,
    val url: String? = null,
    val starts_at: String,
    val ends_at: String,
    val all_day: Boolean = false,
    /** Calendrier de l'espace (« workspace ») : événement Looty modifiable. */
    val event_id: String? = null,
    val recurring: Boolean = false,
    val can_edit: Boolean = false,
)
@Serializable data class CalTask(val id: String, val title: String, val due_date: String, val is_completed: Boolean = false, val board_id: String, val board_name: String = "")
@Serializable data class CalEvents(val events: List<CalEvent> = emptyList(), val tasks: List<CalTask> = emptyList())
@Serializable data class CalendarSource(val id: String, val name: String, val color: String = "#5e6ad2", val kind: String = "ics", val scope: String = "personal", val last_error: String? = null)
@Serializable data class LootyCalendar(val id: String, val name: String = "", val color: String = "#5e6ad2", val is_default: Boolean = false, val board_id: String? = null)
@Serializable data class Repeat(val freq: String, val interval: Int = 1, val until: String? = null)
@Serializable data class EventDetail(
    val id: String,
    val title: String = "",
    val description: String? = null,
    val location: String? = null,
    val start: String,
    val end: String,
    val all_day: Boolean = false,
    val tzid: String? = null,
    val repeat: Repeat? = null,
    val complex_recurrence: Boolean = false,
)

// --- Mail (lecture seule) ---
@Serializable data class MailAccount(val id: String, val label: String, val email: String = "", val webmail_url: String? = null, val unread: Int = 0, val last_error: String? = null, val last_check_at: String? = null)
@Serializable data class MailSummary(val uid: Long, val from_name: String = "", val from_addr: String = "", val subject: String = "", val snippet: String = "", val sent_at: String? = null, val seen: Boolean = false, val has_attachments: Boolean = false)
@Serializable data class MailPage(val messages: List<MailSummary> = emptyList(), val has_more: Boolean = false)
@Serializable data class MailAddress(val name: String? = null, val address: String? = null)
@Serializable data class MailAttachment(val name: String = "", val size: Long = 0, val type: String? = null)
@Serializable data class MailMessage(
    val uid: Long,
    val subject: String? = null,
    val from: List<MailAddress> = emptyList(),
    val to: List<MailAddress> = emptyList(),
    val cc: List<MailAddress> = emptyList(),
    val date: String? = null,
    val html: String? = null,
    val text: String? = null,
    val attachments: List<MailAttachment> = emptyList(),
)
@Serializable data class Created(val id: String)

// --- Tableaux et tâches (G2) ---
@Serializable data class Label(val id: String, val name: String, val color: String = "#5e6ad2")
@Serializable data class Task(
    val id: String,
    val column_id: String,
    val parent_id: String? = null,
    val title: String,
    val description: String? = null,
    val position: Double = 0.0,
    val is_per_person: Boolean = false,
    val is_completed: Boolean = false,
    val require_all_validations: Boolean = true,
    val start_date: String? = null,
    val due_date: String? = null,
    val priority: Int = 0,
    val labels: List<Label> = emptyList(),
    val assignees: List<String> = emptyList(),
    val validations: List<String> = emptyList(),
    val updated_at: String? = null,
)
@Serializable data class ColumnFull(val id: String, val name: String, val position: Double = 0.0, val tasks: List<Task> = emptyList())
@Serializable data class BoardFull(val id: String, val name: String, val columns: List<ColumnFull> = emptyList(), val labels: List<Label> = emptyList()) {
    val allTasks: List<Task> get() = columns.flatMap { it.tasks }
    fun task(id: String): Task? = columns.firstNotNullOfOrNull { c -> c.tasks.firstOrNull { it.id == id } }
    fun column(id: String): ColumnFull? = columns.firstOrNull { it.id == id }
}
@Serializable data class TaskEnvelope(val task: Task, val board_id: String)
@Serializable data class Comment(val id: String, val task_id: String, val user_id: String, val content: String, val created_at: String, val updated_at: String? = null)
@Serializable data class Attachment(val id: String, val filename: String, val file_path: String, val mime_type: String? = null, val size_bytes: Long? = null)
