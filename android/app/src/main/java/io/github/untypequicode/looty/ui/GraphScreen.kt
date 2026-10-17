package io.github.untypequicode.looty.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Graph
import io.github.untypequicode.looty.api.GraphNode
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt
import kotlin.random.Random

/** Disposition « ressorts » : liens qui attirent, points qui se repoussent. */
private fun layout(g: Graph): Map<String, Offset> {
    val ids = g.nodes.map { it.id }
    val index = ids.withIndex().associate { (i, id) -> id to i }
    val n = ids.size
    if (n == 0) return emptyMap()
    val rnd = Random(42)
    val x = FloatArray(n) { rnd.nextFloat() * 1000f - 500f }
    val y = FloatArray(n) { rnd.nextFloat() * 1000f - 500f }
    val edges = g.edges.mapNotNull { e -> val a = index[e.source]; val b = index[e.target]; if (a != null && b != null && a != b) a to b else null }
    val k = 80f
    val iterations = if (n > 600) 120 else 250
    var temp = 200f
    val dx = FloatArray(n); val dy = FloatArray(n)
    repeat(iterations) {
        dx.fill(0f); dy.fill(0f)
        for (i in 0 until n) for (j in i + 1 until n) {
            val ddx = x[i] - x[j]; val ddy = y[i] - y[j]
            val d2 = max(ddx * ddx + ddy * ddy, 1f)
            // Répulsion limitée au voisinage : les points isolés restent près des autres.
            if (d2 > 16 * k * k) continue
            val f = k * k / d2
            dx[i] += ddx * f; dy[i] += ddy * f; dx[j] -= ddx * f; dy[j] -= ddy * f
        }
        for ((a, b) in edges) {
            val ddx = x[a] - x[b]; val ddy = y[a] - y[b]
            val d = max(sqrt(ddx * ddx + ddy * ddy), 0.01f)
            val f = d / k
            dx[a] -= ddx * f; dy[a] -= ddy * f; dx[b] += ddx * f; dy[b] += ddy * f
        }
        for (i in 0 until n) {
            // Légère attraction vers le centre (points isolés).
            dx[i] -= x[i] * 0.05f; dy[i] -= y[i] * 0.05f
            val d = max(sqrt(dx[i] * dx[i] + dy[i] * dy[i]), 0.01f)
            x[i] += dx[i] / d * min(d, temp); y[i] += dy[i] / d * min(d, temp)
        }
        temp *= 0.97f
    }
    return ids.withIndex().associate { (i, id) -> id to Offset(x[i], y[i]) }
}

@Composable
private fun kindColor(kind: String): Color = when (kind) {
    "task" -> Color(0xFF4CB782)
    "board" -> Color(0xFFF2994A)
    "event" -> Color(0xFFE06BA8)
    else -> MaterialTheme.colorScheme.primary
}

/** Graphe des liens entre documents, tâches, tableaux et événements. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GraphScreen(state: AppState, onBack: () -> Unit, onRoute: (String) -> Unit) {
    var graph by remember { mutableStateOf<Graph?>(null) }
    var pos by remember { mutableStateOf<Map<String, Offset>>(emptyMap()) }
    var failed by remember { mutableStateOf(false) }
    var selected by remember { mutableStateOf<GraphNode?>(null) }
    var kinds by remember { mutableStateOf(setOf("page", "task", "board", "event")) }
    var scale by remember { mutableFloatStateOf(1f) }
    var pan by remember { mutableStateOf(Offset.Zero) }
    var canvas by remember { mutableStateOf(androidx.compose.ui.unit.IntSize.Zero) }
    /** Cadre tout le graphe visible dans la zone de dessin. */
    fun fit() {
        val pts = pos.values
        if (pts.isEmpty() || canvas.width == 0) return
        val minX = pts.minOf { it.x }; val maxX = pts.maxOf { it.x }; val minY = pts.minOf { it.y }; val maxY = pts.maxOf { it.y }
        val margin = 64f
        // Place à droite pour les noms écrits à côté des points.
        val labels = min(canvas.width * 0.3f, 360f)
        scale = min((canvas.width - 2 * margin - labels) / max(maxX - minX, 1f), (canvas.height - 2 * margin) / max(maxY - minY, 1f)).coerceIn(0.15f, 6f)
        pan = -Offset((minX + maxX) / 2, (minY + maxY) / 2) * scale - Offset(labels / 2, 0f)
    }
    LaunchedEffect(pos, canvas) { fit() }
    LaunchedEffect(Unit) {
        val g = state.graph()
        if (g == null) { failed = true; return@LaunchedEffect }
        pos = withContext(Dispatchers.Default) { layout(g) }
        graph = g
    }
    val g = graph
    val visible = g?.nodes?.filter { it.kind in kinds }.orEmpty()
    val visibleIds = visible.map { it.id }.toSet()
    val measurer = rememberTextMeasurer()
    val colors = mapOf("page" to kindColor("page"), "task" to kindColor("task"), "board" to kindColor("board"), "event" to kindColor("event"))
    val edgeColor = MaterialTheme.colorScheme.outlineVariant
    val labelColor = MaterialTheme.colorScheme.onSurface
    val highlight = MaterialTheme.colorScheme.tertiary

    Scaffold(topBar = {
        TopAppBar(
            title = { Text(stringResource(R.string.graph)) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            actions = { IconButton(onClick = { fit() }) { Icon(Icons.Outlined.CenterFocusStrong, stringResource(R.string.recenter)) } },
        )
    }) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("page" to R.string.nav_docs, "task" to R.string.nav_tasks, "board" to R.string.boards, "event" to R.string.calendar).forEach { (k, label) ->
                    FilterChip(k in kinds, { kinds = if (k in kinds) kinds - k else kinds + k }, label = { Text(stringResource(label)) },
                        leadingIcon = { Box(Modifier.size(10.dp).then(Modifier.padding(0.dp))) { Canvas(Modifier.fillMaxSize()) { drawCircle(colors[k]!!) } } })
                }
            }
            when {
                failed -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                g == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                g.nodes.isEmpty() -> EmptyState(Icons.Outlined.Hub, stringResource(R.string.graph_empty))
                else -> Canvas(
                    Modifier.fillMaxSize().onSizeChanged { canvas = it }
                        .pointerInput(Unit) { detectTransformGestures { _, p, zoom, _ -> scale = (scale * zoom).coerceIn(0.15f, 6f); pan += p } }
                        .pointerInput(visibleIds) {
                            detectTapGestures { tap ->
                                val c = Offset(size.width / 2f, size.height / 2f)
                                selected = visible.minByOrNull { n -> pos[n.id]?.let { (c + pan + it * scale - tap).getDistance() } ?: Float.MAX_VALUE }
                                    ?.takeIf { n -> (c + pan + pos[n.id]!! * scale - tap).getDistance() < 28.dp.toPx() }
                            }
                        },
                ) {
                    val c = Offset(size.width / 2f, size.height / 2f) + pan
                    fun at(id: String) = pos[id]?.let { c + it * scale }
                    val sel = selected?.id
                    for (e in g.edges) {
                        if (e.source !in visibleIds || e.target !in visibleIds) continue
                        val a = at(e.source) ?: continue; val b = at(e.target) ?: continue
                        val on = sel != null && (e.source == sel || e.target == sel)
                        drawLine(if (on) highlight else edgeColor, a, b, strokeWidth = if (on) 3f else 1.5f)
                    }
                    for (n in visible) {
                        val p = at(n.id) ?: continue
                        if (p.x < -50 || p.y < -50 || p.x > size.width + 50 || p.y > size.height + 50) continue
                        val r = (if (n.kind == "board") 9f else 6f) * scale.coerceIn(0.6f, 2f)
                        drawCircle(colors[n.kind] ?: edgeColor, r, p, alpha = if (n.done) 0.45f else 1f)
                        if (n.id == sel) drawCircle(highlight, r + 6f, p, style = Stroke(3f))
                        // Noms visibles quand on zoome (ou pour le point choisi).
                        // Noms des tâches seulement de près (ils sont nombreux) ; une ligne, sans retour.
                        if (n.id == sel || (n.kind != "task" && scale > 0.6f) || scale > 2.5f) {
                            val text = listOfNotNull(n.icon, n.label).joinToString(" ").take(40)
                            val layout = measurer.measure(text, TextStyle(fontSize = 11.sp, color = labelColor), maxLines = 1, softWrap = false)
                            drawText(layout, topLeft = p + Offset(r + 4f, -layout.size.height / 2f))
                        }
                    }
                }
            }
        }
    }

    selected?.let { n ->
        val links = g?.edges.orEmpty().filter { it.source == n.id || it.target == n.id }
            .mapNotNull { e -> g?.nodes?.firstOrNull { it.id == (if (e.source == n.id) e.target else e.source) }?.let { it to e.kind } }
        ModalBottomSheet(onDismissRequest = { selected = null }) {
            NoNavScrim()
            Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
                ListItem(colors = sheetItemColors(),
                    headlineContent = { Text(listOfNotNull(n.icon, n.label.ifBlank { stringResource(R.string.untitled) }).joinToString(" "), style = MaterialTheme.typography.titleMedium) },
                    supportingContent = { Text(stringResource(kindLabelRes(n.kind))) },
                    trailingContent = { routeOf(n)?.let { r -> FilledTonalButton(onClick = { selected = null; onRoute(r) }) { Text(stringResource(R.string.open)) } } })
                if (links.isNotEmpty()) Text(stringResource(R.string.linked_items), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
                links.forEach { (o, kind) ->
                    ListItem(colors = sheetItemColors(),
                        headlineContent = { Text(listOfNotNull(o.icon, o.label.ifBlank { stringResource(R.string.untitled) }).joinToString(" "), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = { Text(stringResource(kindLabelRes(o.kind)) + " · " + stringResource(when (kind) { "parent" -> R.string.edge_parent; "board" -> R.string.edge_board; else -> R.string.edge_link })) },
                        leadingContent = { Box(Modifier.size(12.dp)) { Canvas(Modifier.fillMaxSize()) { drawCircle(colors[o.kind] ?: Color.Gray) } } },
                        modifier = Modifier.clickable { selected = o })
                }
            }
        }
    }
}

private fun kindLabelRes(kind: String) = when (kind) { "task" -> R.string.kind_task; "board" -> R.string.board; "event" -> R.string.kind_event; else -> R.string.kind_doc }

private fun routeOf(n: GraphNode): String? = when (n.kind) {
    "page" -> "doc/${n.id}"
    "task" -> "task/${n.id}"
    "board" -> "board/${n.id}"
    "event" -> "calendar"
    else -> null
}
