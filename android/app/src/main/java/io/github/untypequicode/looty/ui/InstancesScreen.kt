package io.github.untypequicode.looty.ui

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.Instance
import io.github.untypequicode.looty.InstanceActions
import io.github.untypequicode.looty.Instances
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.SetupActivity
import io.github.untypequicode.looty.switchTo
import kotlinx.coroutines.launch

/**
 * Instances connues de l'app : en changer, en ajouter une, en retirer une (ADR 0017). Chaque instance
 * garde ses données (session, copie hors ligne, notifications, widgets) ; la retirer les efface.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InstancesScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var tick by remember { mutableIntStateOf(0) }
    val instances = remember(tick) { Instances.all(ctx) }
    val active = remember(tick) { Instances.activeId(ctx) }
    var removing by remember { mutableStateOf<Instance?>(null) }
    var busy by remember { mutableStateOf(false) }

    Scaffold(
        topBar = { TopAppBar(title = { Text(stringResource(R.string.instances_title)) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } }) },
    ) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize()) {
            item { Text(stringResource(R.string.instances_hint), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(20.dp)) }
            items(instances, key = { it.id }) { i ->
                val isActive = i.id == active
                ListItem(
                    headlineContent = { Text(i.origin.substringAfter("://")) },
                    supportingContent = { if (isActive) Text(stringResource(R.string.instances_active)) },
                    leadingContent = { Icon(if (isActive) Icons.Outlined.Check else Icons.Outlined.Dns, null) },
                    trailingContent = { IconButton(onClick = { removing = i }, enabled = !busy) { Icon(Icons.Outlined.Delete, stringResource(R.string.instances_remove)) } },
                    modifier = Modifier.clickable(enabled = !isActive && !busy) { Instances.switchTo(ctx, i.id) },
                )
            }
            item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.instances_add)) },
                    leadingContent = { Icon(Icons.Outlined.Add, null) },
                    modifier = Modifier.clickable { ctx.startActivity(Intent(ctx, SetupActivity::class.java)) },
                )
            }
        }
    }

    removing?.let { i ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text(stringResource(R.string.instances_remove_title, i.origin.substringAfter("://"))) },
            text = { Text(stringResource(R.string.instances_remove_body)) },
            confirmButton = {
                TextButton(onClick = {
                    removing = null
                    busy = true
                    scope.launch {
                        val next = InstanceActions.remove(ctx, i.id)
                        busy = false
                        when {
                            // Plus aucune instance : retour à la saisie du serveur.
                            next == null -> ctx.startActivity(Intent(ctx, SetupActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
                            // L'instance retirée était celle de l'écran : repartir sur l'active.
                            i.id == active -> Instances.switchTo(ctx, next.id)
                            else -> tick++
                        }
                    }
                }) { Text(stringResource(R.string.instances_remove)) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
