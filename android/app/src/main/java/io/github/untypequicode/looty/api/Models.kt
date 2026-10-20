package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

@Serializable data class User(val id: String, val name: String = "", val email: String = "", val avatar_url: String? = null, val theme: String? = null, val locale: String? = null, val is_super_admin: Boolean = false)
@Serializable data class Me(val authenticated: Boolean = false, val user: User? = null)
@Serializable data class Workspace(val id: String, val name: String, val role: String = "member")
@Serializable data class Workspaces(val workspaces: List<Workspace> = emptyList())
@Serializable data class Member(val user_id: String, val name: String = "", val email: String = "", val avatar_url: String? = null, val role: String = "", val role_id: String? = null, val role_name: String? = null, val role_color: String? = null, val joined_at: String? = null)
/** Droits du rôle dans l’espace (GET access/me) : none < read < comment < validate < edit < manage. */
@Serializable data class Permissions(val docs: String = "edit", val boards: String = "edit", val calendar: String = "edit", val invite: Boolean = false, val share: Boolean = false, val admin: Boolean = false)
@Serializable data class MyAccess(val role: String = "member", val role_name: String? = null, val permissions: Permissions = Permissions(), val group_tasks: Boolean = true)
@Serializable data class Members(val members: List<Member> = emptyList())
/** Groupe de membres (sans droit) : mentions @groupe, ajout de tous ses membres d’un coup. */
@Serializable data class MemberGroup(val id: String, val name: String, val color: String = "#5e6ad2", val members: List<String> = emptyList())

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
    /** La personne peut ajouter des sous-pages ici (droit accordé sur la page). */
    val can_add_child: Boolean? = null,
    val share_token: String? = null,
    val is_public: Boolean? = null,
    val updated_by: String? = null,
    /** HTML du document (lecture seule dans l’application). */
    val content: String? = null,
)
/** Commentaire d'un document (fil : parent_id null ; réponses : parent_id du fil). */
@Serializable data class DocComment(
    val id: String, val parent_id: String? = null, val user_id: String? = null, val user_name: String? = null,
    val content: String = "", val quote: String? = null, val resolved_at: String? = null, val created_at: String, val updated_at: String? = null,
)
/** Version d'un document (instantané de l'éditeur collaboratif). */
@Serializable data class Revision(val id: String, val kind: String = "auto", val label: String? = null, val title: String = "", val created_by: String? = null,
    val author_name: String? = null, val created_at: String, val updated_at: String? = null, val size: Int = 0)
@Serializable data class PageRef(val id: String, val title: String = "")
/** Accès particulier d'une personne à une page (viewer, editor, blocked). */
@Serializable data class PagePerm(val user_id: String, val name: String = "", val email: String = "", val role: String = "viewer", val inherit: Boolean = true,
    val allow_subpages: Boolean = false, val is_member: Boolean = true, val inherited_from: PageRef? = null)

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

// --- Mail ---
@Serializable data class MailAccount(
    val id: String, val label: String, val email: String = "", val webmail_url: String? = null, val unread: Int = 0,
    val last_error: String? = null, val last_check_at: String? = null,
    val imap_host: String = "", val imap_port: Int = 993, val security: String = "tls", val username: String = "", val notify: Boolean = true,
    val display_name: String? = null, val signature: String? = null,
    val smtp_host: String? = null, val smtp_port: Int? = null, val smtp_security: String? = null, val smtp_username: String? = null,
    /** Envoi configuré (SMTP). */
    val can_send: Boolean = false,
)
/** Message d'une liste ; `to`, `flagged`, `answered` seulement pour les dossiers lus en direct. */
@Serializable data class MailSummary(
    val uid: Long, val from_name: String = "", val from_addr: String = "", val to: List<String> = emptyList(), val subject: String = "", val snippet: String = "",
    val sent_at: String? = null, val seen: Boolean = false, val flagged: Boolean = false, val answered: Boolean = false, val has_attachments: Boolean = false,
)
@Serializable data class MailPage(val messages: List<MailSummary> = emptyList(), val has_more: Boolean = false)
@Serializable data class MailFolder(val name: String, val label: String = "", val parent: String? = null, val role: String? = null, val selectable: Boolean = true, val total: Int = 0, val unread: Int = 0)
@Serializable data class MailFolders(val delimiter: String = "/", val folders: List<MailFolder> = emptyList())
@Serializable data class MailAddress(val name: String? = null, val address: String? = null)
@Serializable data class MailAttachment(val index: Int = -1, val name: String = "", val size: Long = 0, val type: String? = null)
@Serializable data class MailMessage(
    val uid: Long,
    val folder: String = "INBOX",
    val subject: String? = null,
    val from: List<MailAddress> = emptyList(),
    val to: List<MailAddress> = emptyList(),
    val cc: List<MailAddress> = emptyList(),
    val reply_to: List<MailAddress> = emptyList(),
    val date: String? = null,
    val message_id: String? = null,
    val references: List<String> = emptyList(),
    val flagged: Boolean = false,
    val html: String? = null,
    val text: String? = null,
    val attachments: List<MailAttachment> = emptyList(),
)
@Serializable data class Contact(val name: String? = null, val address: String)
@Serializable data class Uploaded(val url: String, val name: String = "", val mime: String? = null, val size: Long = 0)
@Serializable data class SendResult(val warning: String? = null)
@Serializable data class DeleteResult(val permanent: Boolean = false)
@Serializable data class FolderCreated(val name: String = "")
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
@Serializable data class BoardFull(val id: String, val name: String, val columns: List<ColumnFull> = emptyList(), val labels: List<Label> = emptyList(), val automations: JsonObject? = null) {
    val allTasks: List<Task> get() = columns.flatMap { it.tasks }
    fun task(id: String): Task? = columns.firstNotNullOfOrNull { c -> c.tasks.firstOrNull { it.id == id } }
    fun column(id: String): ColumnFull? = columns.firstOrNull { it.id == id }
}
@Serializable data class TaskEnvelope(val task: Task, val board_id: String)
@Serializable data class Comment(val id: String, val task_id: String, val user_id: String, val content: String, val created_at: String, val updated_at: String? = null)
/** Historique d'une tâche (valeurs avant / après selon le type). */
@Serializable data class TaskActivity(val id: String, val task_id: String, val user_id: String, val activity_type: String, val old_value: JsonElement? = null, val new_value: JsonElement? = null, val created_at: String)
@Serializable data class Attachment(val id: String, val filename: String, val file_path: String, val mime_type: String? = null, val size_bytes: Long? = null)

// --- Drive (fichiers temporaires de l'espace) ---
@Serializable data class DriveItem(
    val id: String, val folder: String = "", val name: String, val is_dir: Boolean = false, val mime: String? = null, val size: Long = 0,
    val created_at: String? = null, val expires_at: String? = null, val uploaded_by: String? = null, val uploaded_by_name: String? = null,
    val shared: Boolean = false, val children: Long = 0,
)
@Serializable data class DriveList(
    val folder: String = "", val items: List<DriveItem> = emptyList(), val max_days: Int = 30, val quota_bytes: Long = 0, val used_bytes: Long = 0,
    val day_choices: List<Int> = emptyList(), val can_upload: Boolean = false, val can_manage: Boolean = false, val user_id: String = "",
)
@Serializable data class ShareLink(val url: String, val expires_at: String? = null)

// --- Projets (mini-sites, LaTeX, notebooks) ---
@Serializable data class ProjectSummary(
    val id: String, val kind: String = "site", val name: String = "", val slug: String? = null, val published: Boolean = false,
    val updated_at: String? = null, val last_build_ok: Boolean? = null, val files: Long = 0, val created_by_name: String? = null,
)
@Serializable data class ProjectInfo(val id: String, val kind: String = "site", val name: String = "", val slug: String? = null, val published: Boolean = false, val settings: JsonObject? = null)
@Serializable data class ProjectFile(val id: String, val path: String, val is_dir: Boolean = false, val text: Boolean = false, val mime: String? = null, val size: Long = 0,
    val updated_at: String? = null, val updated_by_name: String? = null)
@Serializable data class ProjectFull(val project: ProjectInfo, val files: List<ProjectFile> = emptyList(), val public_url: String? = null,
    val can_write: Boolean = false, val can_manage: Boolean = false, val can_publish: Boolean = false)
@Serializable data class CompileIssue(val file: String? = null, val line: Int? = null, val message: String = "", val level: String = "error")
@Serializable data class CompileResult(val ok: Boolean = false, val pdf: Boolean = false, val engine: String? = null, val duration_ms: Long = 0,
    val issues: List<CompileIssue> = emptyList(), val log: String = "")
@Serializable data class PreviewBase(val base: String)

// --- Activité de l'espace ---
@Serializable data class FeedItem(
    val id: String, val source: String = "task", val type: String = "", val created_at: String, val user_id: String = "", val user_name: String? = null,
    val target_id: String = "", val title: String = "", val icon: String? = null, val board_id: String? = null, val board_name: String? = null,
    val new_value: JsonElement? = null, val snippet: String? = null,
)
@Serializable data class Feed(val items: List<FeedItem> = emptyList(), val has_more: Boolean = false)

// --- Corbeille ---
@Serializable data class TrashedTask(val id: String, val title: String = "", val board_id: String = "", val board_name: String = "", val deleted_at: String,
    val deleted_by: String? = null, val deleted_by_name: String? = null, val subtasks: Long = 0, val expires_at: String? = null)
@Serializable data class TrashedPage(val id: String, val title: String = "", val icon: String? = null, val parent_title: String? = null, val deleted_at: String,
    val deleted_by: String? = null, val deleted_by_name: String? = null, val subpages: Long = 0, val expires_at: String? = null)
@Serializable data class Trash(val retention_days: Int = 30, val can_manage: Boolean = false, val tasks: List<TrashedTask> = emptyList(), val pages: List<TrashedPage> = emptyList())

// --- Graphe des liens ---
@Serializable data class GraphNode(val id: String, val kind: String = "page", val label: String = "", val icon: String? = null, val done: Boolean = false, val board_id: String? = null)
@Serializable data class GraphEdge(val source: String, val target: String, val kind: String = "link", val id: String? = null)
@Serializable data class Graph(val nodes: List<GraphNode> = emptyList(), val edges: List<GraphEdge> = emptyList())

// --- Moodle ---
@Serializable data class MoodleConnection(val base_url: String = "", val full_name: String? = null, val site_name: String? = null, val last_error: String? = null,
    val last_sync_at: String? = null, val calendar_id: String? = null, val notify: Boolean = true,
    /** Site ouvert déjà connecté (sinon : se reconnecter une fois). Vrai par défaut pour les anciens serveurs. */
    val autologin: Boolean = true)
@Serializable data class MoodleOverview(val url: String? = null, val default_url: String = "", val can_configure: Boolean = false, val connection: MoodleConnection? = null)
@Serializable data class MoodleSite(val base_url: String = "", val site_name: String? = null, val sso: Boolean = true, val launch_url: String? = null, val tokens_url: String? = null)
@Serializable data class MoodleCourse(val id: Long, val fullname: String? = null, val shortname: String? = null, val category: String? = null, val progress: Double? = null,
    val favourite: Boolean = false, val url: String? = null)
@Serializable data class MoodleDeadline(val id: Long, val name: String? = null, val course: String? = null, val module: String? = null, val due: String? = null,
    val overdue: Boolean = false, val url: String? = null, val action: String? = null)
@Serializable data class MoodleFile(val name: String = "", val path: String = "/", val size: Long = 0, val mime: String? = null, val time: String? = null,
    val url: String? = null, val html: Boolean = false, val source: String = "")
@Serializable data class MoodleDate(val label: String? = null, val time: String? = null)
@Serializable data class MoodleModule(val id: Long, val name: String = "", val type: String = "", val type_name: String? = null, val url: String = "",
    val description: String = "", val files: List<MoodleFile> = emptyList(), val link: String? = null, val dates: List<MoodleDate> = emptyList(), val done: Boolean? = null)
@Serializable data class MoodleSection(val id: Long, val name: String? = null, val summary: String = "", val modules: List<MoodleModule> = emptyList())
@Serializable data class MoodleCourseContent(val id: Long, val url: String = "", val sections: List<MoodleSection> = emptyList())
@Serializable data class MoodleAttachment(val name: String = "", val size: Long = 0, val mime: String? = null, val url: String? = null)
@Serializable data class MoodleAssignment(val id: Long, val cmid: Long, val name: String = "", val intro: String = "", val url: String = "",
    val attachments: List<MoodleAttachment> = emptyList(), val due: String? = null, val cutoff: String? = null,
    val status: String? = null, val grading: String? = null, val grade: String? = null)
@Serializable data class MoodleHtml(val html: String = "")
@Serializable data class MoodleNotif(val id: Long, val subject: String? = null, val text: String? = null, val url: String? = null, val read: Boolean = false, val time: String? = null)
@Serializable data class MoodleNotifs(val notifications: List<MoodleNotif> = emptyList(), val unread: Int = 0)
@Serializable data class MoodleConversation(val id: Long, val name: String = "", val group: Boolean = false, val unread: Int = 0, val last_text: String? = null,
    val last_from_me: Boolean? = null, val last_time: String? = null)
@Serializable data class MoodleMessage(val id: Long, val mine: Boolean = false, val from: String = "", val html: String = "", val time: String? = null)
@Serializable data class MoodleThread(val id: Long, val messages: List<MoodleMessage> = emptyList())
@Serializable data class OpenUrl(val url: String)

// --- Réglages ---
@Serializable data class EventPref(val event: String, val email: Boolean = false, val push: Boolean = true)
@Serializable data class NotifPrefs(val email_enabled: Boolean = false, val events: List<EventPref> = emptyList(), val email_available: Boolean = false, val email: String = "")
@Serializable data class PushDevice(val id: String, val device: String? = null, val endpoint_host: String = "", val created_at: String? = null, val last_used_at: String? = null)
@Serializable data class ApiToken(val id: String, val name: String = "", val scope: String = "api", val created_at: String, val last_used_at: String? = null)
@Serializable data class NewToken(val token: String)
@Serializable data class Identity(val provider: String, val provider_name: String? = null, val created_at: String)
@Serializable data class WsRole(val id: String, val name: String = "", val color: String = "#8a8f98", val preset: String? = null, val member_count: Long = 0, val permissions: Permissions = Permissions())
@Serializable data class Invite(val id: String, val role: String = "", val created_by_name: String? = null, val expires_at: String? = null, val max_uses: Int? = null, val uses: Int = 0, val created_at: String)
@Serializable data class UserHit(val id: String, val name: String = "", val email: String = "", val avatar_url: String? = null)
@Serializable data class AddResult(val added: String? = null, val invited: String? = null, val email_sent: Boolean = false, val link: String? = null)
@Serializable data class CalDavAccount(val id: String, val name: String = "", val server_url: String = "", val calendar_url: String = "", val calendar_name: String? = null,
    val calendar_color: String? = null, val username: String = "", val board_ids: List<String>? = null, val include_completed: Boolean = false, val enabled: Boolean = true,
    val last_sync_at: String? = null, val last_error: String? = null, val task_count: Long = 0)
@Serializable data class DiscoveredCalendar(val url: String, val name: String = "", val color: String? = null, val supports_todo: Boolean = true)
@Serializable data class SyncReport(val pushed: Int = 0, val pulled: Int = 0, val deleted: Int = 0)
@Serializable data class SyncResult(val report: SyncReport = SyncReport())
@Serializable data class WorkspaceDrive(val max_days: Int = 30, val quota_bytes: Long = 0)
