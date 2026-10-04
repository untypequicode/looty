package io.github.untypequicode.looty

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.concurrent.thread

/** Premier lancement (ou « Changer de serveur ») : adresse du serveur Looty. */
class SetupActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val current = Config.server(this) ?: ""
        setContent {
            val dark = isSystemInDarkTheme()
            val accent = Color(0xFF5E6AD2)
            MaterialTheme(colorScheme = if (dark) darkColorScheme(primary = accent, background = Color(0xFF0B0B0D), surface = Color(0xFF0B0B0D)) else lightColorScheme(primary = accent, background = Color(0xFFFCFCFD), surface = Color(0xFFFCFCFD))) {
                Surface(Modifier.fillMaxSize()) { Setup(current, ::done) }
            }
        }
    }

    private fun done(origin: String) {
        Config.setServer(this, origin)
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_NEW_TASK).putExtra(MainActivity.EXTRA_RELOAD, true))
        finish()
    }
}

@Composable
private fun Setup(initial: String, onDone: (String) -> Unit) {
    var url by remember { mutableStateOf(initial.removePrefix("https://")) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<Int?>(null) }

    fun submit() {
        val origin = Config.normalize(url)
        if (origin == null) { error = R.string.setup_not_looty; return }
        if (origin.startsWith("http://") && !Config.cleartextAllowed(origin)) { error = R.string.setup_https; return }
        busy = true
        error = null
        thread {
            var target = origin
            var h = Config.health(origin)
            // Sans « http(s):// » : un serveur local (réseau domestique) peut n'être qu'en http.
            val plain = origin.replaceFirst("https://", "http://")
            if (h == Config.Health.Unreachable && !url.contains("://") && Config.cleartextAllowed(plain)) {
                h = Config.health(plain)
                target = plain
            }
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                busy = false
                when (h) {
                    is Config.Health.Ok -> onDone(target)
                    Config.Health.NotLooty -> error = R.string.setup_not_looty
                    Config.Health.Unreachable -> error = R.string.setup_unreachable
                }
            }
        }
    }

    Column(
        Modifier.fillMaxSize().systemBarsPadding().imePadding().padding(28.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.Start,
    ) {
        Text(stringResource(R.string.setup_title), fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.setup_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(24.dp))
        OutlinedTextField(
            value = url,
            onValueChange = { url = it; error = null },
            label = { Text(stringResource(R.string.setup_url)) },
            placeholder = { Text("looty.example.org") },
            singleLine = true,
            isError = error != null,
            supportingText = error?.let { e -> { Text(stringResource(e)) } },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go, autoCorrectEnabled = false),
            keyboardActions = KeyboardActions(onGo = { if (!busy) submit() }),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(16.dp))
        Button(onClick = ::submit, enabled = !busy && url.isNotBlank(), modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Text(stringResource(if (busy) R.string.setup_checking else R.string.setup_connect))
        }
    }
}
