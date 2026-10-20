package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R

/**
 * Texte d'un commentaire avec @mentions : taper « @ » propose les membres et les groupes ;
 * ceux choisis deviennent des mentions à l'envoi (un groupe notifie chacun de ses membres).
 */
class MentionInput(initial: String = "", known: List<Html.Mention> = emptyList()) {
    var value by mutableStateOf(TextFieldValue(initial, TextRange(initial.length)))
    val mentions = mutableStateListOf<Html.Mention>().apply { addAll(known) }
    val text get() = value.text

    fun clear() { value = TextFieldValue(""); mentions.clear() }

    /** Recherche en cours (texte après « @ » juste avant le curseur), ou null. */
    fun query(): String? {
        val before = value.text.take(value.selection.start)
        return Regex("(^|\\s)@([^\\s@]{0,30})$").find(before)?.groupValues?.get(2)
    }

    fun pick(m: Html.Mention) {
        val q = query() ?: return
        val end = value.selection.start
        val start = end - q.length - 1
        val inserted = "@${m.label} "
        val text = value.text.substring(0, start) + inserted + value.text.substring(end)
        value = TextFieldValue(text, TextRange(start + inserted.length))
        if (mentions.none { it.id == m.id }) mentions.add(m)
    }
}

/** Liste des membres et groupes qui correspondent à la recherche « @… » du champ. */
@Composable
fun MentionSuggestions(state: AppState, input: MentionInput, modifier: Modifier = Modifier) {
    val q = fold(input.query() ?: return)
    val people = state.members.values.filter { fold(it.name).contains(q) }.sortedBy { it.name.lowercase() }.take(5)
    val groups = state.groups.filter { it.members.isNotEmpty() && fold(it.name).contains(q) }.take(4)
    if (people.isEmpty() && groups.isEmpty()) return
    Surface(modifier.fillMaxWidth(), tonalElevation = 6.dp, shape = MaterialTheme.shapes.medium) {
        Column(Modifier.padding(vertical = 4.dp)) {
            people.forEach { m ->
                Row(Modifier.fillMaxWidth().clickable { input.pick(Html.Mention(m.user_id, m.name)) }.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(m.name, 26.dp); Spacer(Modifier.width(10.dp)); Text(m.name, Modifier.weight(1f))
                }
            }
            if (groups.isNotEmpty()) Text(stringResource(R.string.member_groups), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 12.dp, top = 6.dp))
            groups.forEach { g ->
                Row(Modifier.fillMaxWidth().clickable { input.pick(Html.Mention(g.id, g.name)) }.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    GroupBadge(g.color, 26)
                    Spacer(Modifier.width(10.dp))
                    Text(g.name, Modifier.weight(1f))
                    Text(pluralStringResource(R.plurals.group_members, g.members.size, g.members.size), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

/** Texte comparable sans casse ni accents (« Équipe » est trouvé en tapant « equ »). */
private fun fold(s: String) = java.text.Normalizer.normalize(s, java.text.Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "").lowercase()

/** Pastille « @ » d'un groupe, à sa couleur. */
@Composable
fun GroupBadge(color: String, size: Int = 32) {
    val c = runCatching { Color(android.graphics.Color.parseColor(color)) }.getOrDefault(MaterialTheme.colorScheme.primary)
    Box(Modifier.size(size.dp).clip(CircleShape).background(c), contentAlignment = Alignment.Center) {
        Text("@", color = Color.White, fontWeight = FontWeight.Bold, fontSize = (size * 0.45f).sp)
    }
}
