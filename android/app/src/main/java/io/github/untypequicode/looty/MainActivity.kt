package io.github.untypequicode.looty

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.material.icons.outlined.LinkOff
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch
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
    /** Lien reçu d'une autre instance que celle de l'app, en attente d'un choix. */
    private var foreign by mutableStateOf<InstanceLinks.LinkAction.Foreign?>(null)

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
            // Lien d'une instance alors que l'app n'en a pas : se connecter à celle-là, puis ouvrir la page.
            val connect = linkAction(intent) as? InstanceLinks.LinkAction.Connect
            startActivity(Intent(this, SetupActivity::class.java).apply {
                if (connect != null) putExtra(SetupActivity.EXTRA_SERVER, connect.instance).putExtra(SetupActivity.EXTRA_PATH, connect.path)
            })
            finish()
            return
        }
        if (savedInstanceState == null) state.start()
        handleLink(intent)
        setContent {
            LootyTheme {
                Surface(Modifier.fillMaxSize()) {
                    val f = foreign
                    if (f != null) {
                        BackHandler { foreign = null }
                        ForeignLink(f, onBrowser = { openInBrowser(f.webUrl); foreign = null }, onClose = { foreign = null })
                    } else when (val s = state.session) {
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
        if (state.session is Session.Ready) {
            state.refreshAll()
            // Modifications faites hors ligne : envoyées dès le retour dans l'application.
            state.viewModelScope.launch { state.drain() }
        }
    }

    private fun linkAction(intent: Intent?): InstanceLinks.LinkAction? =
        intent?.data?.let { InstanceLinks.resolve(it.toString(), Config.server(this)) }

    /** looty://w/… : la page, si le lien est de l'instance de l'app ; sinon l'écran « autre instance ». */
    private fun handleLink(intent: Intent?) {
        val uri = intent?.data ?: return
        if (uri.scheme == "looty" && uri.host == "app") { state.pendingLink = "/app" + (uri.encodedPath ?: ""); return }
        when (val a = linkAction(intent)) {
            // Ouvert par l'application dès que la session est prête (écran natif quand il existe).
            is InstanceLinks.LinkAction.Open -> { foreign = null; state.pendingLink = a.path }
            is InstanceLinks.LinkAction.Foreign -> foreign = a
            is InstanceLinks.LinkAction.Connect, null -> {}
        }
    }

    private fun openInBrowser(url: String) {
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE))
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(this, url, Toast.LENGTH_LONG).show()
        }
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

/** « https://autre.fr » → « autre.fr » (l'origine entière si seul le schéma diffère). */
private fun shown(origin: String, other: String): String =
    origin.substringAfter("://").let { if (it == other.substringAfter("://")) origin else it }

/** Lien d'une autre instance : l'app ne l'ouvre pas, le navigateur le peut (ADR 0004). */
@Composable
private fun ForeignLink(link: InstanceLinks.LinkAction.Foreign, onBrowser: () -> Unit, onClose: () -> Unit) {
    Column(Modifier.fillMaxSize().systemBarsPadding().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(Icons.Outlined.LinkOff, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.outline)
        Spacer(Modifier.height(12.dp))
        Text(stringResource(R.string.link_foreign_title), style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        Spacer(Modifier.height(8.dp))
        Text(
            stringResource(R.string.link_foreign_body, shown(link.linkInstance, link.appInstance), shown(link.appInstance, link.linkInstance)),
            color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(24.dp))
        Button(onClick = onBrowser, modifier = Modifier.fillMaxWidth().height(52.dp)) { Text(stringResource(R.string.link_open_browser)) }
        Spacer(Modifier.height(8.dp))
        TextButton(onClick = onClose) { Text(stringResource(R.string.close)) }
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
