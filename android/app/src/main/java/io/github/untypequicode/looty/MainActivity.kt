package io.github.untypequicode.looty

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.ui.LoadingIndicatorCompat
import io.github.untypequicode.looty.ui.LootyApp
import io.github.untypequicode.looty.ui.LootyTheme

/**
 * Application native (Compose, Material 3 Expressive). Les documents et les écrans pas
 * encore natifs s'ouvrent dans WebActivity ; la connexion SSO aussi.
 */
class MainActivity : ComponentActivity() {
    companion object {
        const val EXTRA_RELOAD = "reload"
    }

    private val state: AppState by viewModels()
    private var loginShown = false

    private val login = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
        loginShown = false
        state.start()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Pas de voile derrière la barre de navigation : la barre du bas de l'app la prolonge.
        if (android.os.Build.VERSION.SDK_INT >= 29) window.isNavigationBarContrastEnforced = false
        if (Config.server(this) == null) {
            startActivity(Intent(this, SetupActivity::class.java))
            finish()
            return
        }
        if (savedInstanceState == null) state.start()
        handleLink(intent)
        setContent {
            LootyTheme {
                Surface(Modifier.fillMaxSize()) {
                    when (val s = state.session) {
                        is Session.Ready -> LootyApp(state) { openLogin() }
                        is Session.NeedLogin -> SignIn { openLogin() }
                        is Session.Offline -> Offline { state.start() }
                        Session.NeedServer -> {}
                        Session.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.getBooleanExtra(EXTRA_RELOAD, false)) state.start()
        handleLink(intent)
    }

    override fun onResume() {
        super.onResume()
        // Retour d'un écran web : les données ont pu changer.
        if (state.session is Session.Ready) state.refreshAll()
    }

    /** looty://w/… : la page correspondante (écran web pour l'instant). */
    private fun handleLink(intent: Intent?) {
        val uri = intent?.data ?: return
        if (uri.scheme != "looty" || uri.host != "w") return
        val path = "/w" + (uri.encodedPath ?: "") + (uri.encodedQuery?.let { "?$it" } ?: "")
        WebActivity.open(this, path)
    }

    private fun openLogin() {
        if (loginShown) return
        loginShown = true
        login.launch(Intent(this, WebActivity::class.java).putExtra(WebActivity.EXTRA_PATH, "/login").putExtra(WebActivity.EXTRA_LOGIN, true))
    }
}

@Composable
private fun SignIn(onSignIn: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(stringResource(R.string.sign_in_title), style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.sign_in_hint), color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
        Spacer(Modifier.height(24.dp))
        Button(onClick = onSignIn, modifier = Modifier.fillMaxWidth().height(52.dp)) { Text(stringResource(R.string.sign_in)) }
    }
}

@Composable
private fun Offline(onRetry: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(Icons.Outlined.CloudOff, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.outline)
        Spacer(Modifier.height(12.dp))
        Text(stringResource(R.string.offline_title), style = MaterialTheme.typography.titleLarge)
        Text(stringResource(R.string.offline_hint), color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
        Spacer(Modifier.height(20.dp))
        Button(onClick = onRetry) { Text(stringResource(R.string.retry)) }
    }
}
