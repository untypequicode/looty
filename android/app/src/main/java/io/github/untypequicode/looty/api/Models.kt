package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable data class User(val id: String, val name: String = "", val email: String = "", val avatar_url: String? = null)
@Serializable data class Me(val authenticated: Boolean = false, val user: User? = null)
@Serializable data class Workspace(val id: String, val name: String, val role: String = "member")
@Serializable data class Workspaces(val workspaces: List<Workspace> = emptyList())
@Serializable data class Member(val user_id: String, val name: String = "", val email: String = "", val avatar_url: String? = null, val role: String = "")
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
    val location: String? = null,
    val starts_at: String,
    val ends_at: String,
    val all_day: Boolean = false,
)
@Serializable data class CalEvents(val events: List<CalEvent> = emptyList())
@Serializable data class Created(val id: String)
