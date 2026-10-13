package io.github.untypequicode.looty

import android.annotation.SuppressLint
import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.ContentValues
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.os.Message
import android.provider.MediaStore
import android.util.Base64
import android.view.ViewGroup
import android.webkit.*
import android.widget.FrameLayout
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.File
import java.security.MessageDigest
import java.security.SecureRandom
import kotlin.concurrent.thread

/**
 * Écran web de l'application native : documents (éditeur collaboratif) et écrans pas
 * encore natifs. Une seule instance (singleTask), au-dessus de l'écran natif ; le geste
 * retour revient à l'écran natif avec l'animation prédictive du système.
 *
 * Looty dans une WebView (moteur Chromium du système) :
 * - seul le serveur choisi s'ouvre dans l'application, le reste dans le navigateur ;
 * - connexion SSO dans le navigateur (clés d'accès, WebAuthn), retour par looty://sso ;
 * - envoi de fichiers, téléchargements, liens looty://w/….
 */
class WebActivity : ComponentActivity() {
    companion object {
        const val EXTRA_PATH = "path"
        /** Connexion : l'écran se ferme (RESULT_OK) dès qu'on arrive dans Looty. */
        const val EXTRA_LOGIN = "login"

        fun open(ctx: android.content.Context, path: String) {
            ctx.startActivity(Intent(ctx, WebActivity::class.java).putExtra(EXTRA_PATH, path))
        }
    }

    private var loginMode = false
    private lateinit var back: OnBackPressedCallback

    private lateinit var web: WebView
    private lateinit var root: FrameLayout
    private lateinit var server: String
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var androidSso = true
    /** Connexion dans la WebView (serveur trop ancien pour la connexion par le navigateur). */
    @Volatile private var signingInUntil = 0L
    private var fileCallback: ValueCallback<Array<Uri>>? = null

    private val picker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        val data = r.data
        val uris = when {
            r.resultCode != RESULT_OK -> null
            data?.clipData != null -> Array(data.clipData!!.itemCount) { data.clipData!!.getItemAt(it).uri }
            data?.data != null -> arrayOf(data.data!!)
            else -> null
        }
        fileCallback?.onReceiveValue(uris)
        fileCallback = null
    }

    /** Connexion à Moodle : la clé (moodlemobile://token=…) revient à la page, comme dans l’application de bureau. */
    private val moodleLogin = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        val link = if (r.resultCode == RESULT_OK) r.data?.getStringExtra(MoodleLoginActivity.EXTRA_LINK) else null
        web.evaluateJavascript("window.dispatchEvent(new CustomEvent('looty:moodle-token', { detail: ${JSONObject.quote(link ?: "")} }))", null)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        server = Config.server(this) ?: run {
            finish()
            return
        }
        loginMode = intent.getBooleanExtra(EXTRA_LOGIN, false)
        enableEdgeToEdge()
        if (Build.VERSION.SDK_INT >= 29) window.isNavigationBarContrastEnforced = false
        root = FrameLayout(this)
        web = WebView(this)
        root.addView(web, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        setContentView(root)
        // Barres système et clavier : la page occupe l'espace restant.
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
            WindowInsetsCompat.CONSUMED
        }
        applyTheme(isNight())
        setupWebView()
        // Retour : historique de la page tant qu'il y en a ; ensuite le système ferme
        // l'écran (animation prédictive vers l'écran natif).
        back = object : OnBackPressedCallback(false) {
            override fun handleOnBackPressed() {
                if (web.canGoBack()) web.goBack()
            }
        }
        onBackPressedDispatcher.addCallback(this, back)
        thread { (Config.health(server) as? Config.Health.Ok)?.let { androidSso = it.androidSso } }

        if (!handleIntent(intent)) web.loadUrl("$server/")
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.hasExtra(EXTRA_PATH)) {
            loginMode = intent.getBooleanExtra(EXTRA_LOGIN, false)
            web.clearHistory()
        }
        handleIntent(intent)
    }

    override fun onPause() {
        super.onPause()
        CookieManager.getInstance().flush()
    }

    /** Connexion terminée : retour à l'écran natif. */
    private fun checkLoggedIn(url: String?) {
        if (!loginMode || url == null) return
        val uri = Uri.parse(url)
        val path = uri.path.orEmpty()
        if (sameOrigin(uri) && path.startsWith("/w/") || (sameOrigin(uri) && path == "/")) {
            CookieManager.getInstance().flush()
            setResult(RESULT_OK)
            finish()
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        applyTheme(isNight())
    }

    private fun isNight() = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES

    /** Couleur de fond et icônes des barres système selon le thème de la page. */
    private fun applyTheme(dark: Boolean) {
        root.setBackgroundColor(if (dark) Color.rgb(11, 11, 13) else Color.rgb(252, 252, 253))
        WindowCompat.getInsetsController(window, root).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
    }

    // ---------------------------------------------------------------------
    // WebView
    // ---------------------------------------------------------------------

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupWebView() {
        WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG)
        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            allowFileAccess = false
            allowContentAccess = false
            setSupportMultipleWindows(true)
            javaScriptCanOpenWindowsAutomatically = true
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            userAgentString = "$userAgentString LootyAndroid/${BuildConfig.VERSION_NAME}"
        }
        CookieManager.getInstance().setAcceptCookie(true)
        web.setBackgroundColor(Color.TRANSPARENT)

        // Messages de la page (origine du serveur uniquement, jamais les cadres intégrés).
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(web, "LootyApp", setOf(server)) { _, msg, _, _, _ -> onPageMessage(msg.data ?: "") }
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(web, PAGE_SCRIPT, setOf(server))
        }

        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
                if (request.isForMainFrame) !allowInApp(request.url, newWindow = false) else false

            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? = intercept(request)

            override fun doUpdateVisitedHistory(view: WebView, url: String?, isReload: Boolean) {
                back.isEnabled = view.canGoBack()
                checkLoggedIn(url)
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (request.isForMainFrame && sameOrigin(request.url)) {
                    failedUrl = request.url.toString()
                    view.loadDataWithBaseURL("$server/__looty/offline", Pages.offline(server), "text/html", "utf-8", null)
                }
            }
        }

        web.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams): Boolean {
                fileCallback?.onReceiveValue(null)
                fileCallback = callback
                val intent = params.createIntent().apply {
                    if (params.mode == FileChooserParams.MODE_OPEN_MULTIPLE) putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                }
                return try {
                    picker.launch(intent)
                    true
                } catch (_: ActivityNotFoundException) {
                    fileCallback = null
                    false
                }
            }

            // window.open / target=_blank : on capte l'adresse, jamais de seconde fenêtre.
            override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message): Boolean {
                val popup = WebView(this@WebActivity)
                popup.webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(v: WebView, request: WebResourceRequest): Boolean {
                        if (allowInApp(request.url, newWindow = true)) web.loadUrl(request.url.toString())
                        v.destroy()
                        return true
                    }
                }
                (resultMsg.obj as WebView.WebViewTransport).webView = popup
                resultMsg.sendToTarget()
                return true
            }
        }

        web.setDownloadListener { url, userAgent, contentDisposition, mimeType, _ ->
            when {
                url.startsWith("blob:") || url.startsWith("data:") ->
                    web.evaluateJavascript("window.__lootySave && __lootySave(${JSONObject.quote(url)}, ${JSONObject.quote(URLUtil.guessFileName(url, contentDisposition, mimeType))})", null)
                else -> download(url, userAgent, contentDisposition, mimeType)
            }
        }
    }

    private var failedUrl: String? = null

    private fun sameOrigin(uri: Uri): Boolean = Config.normalize(uri.toString()) == server && uri.scheme in setOf("http", "https")

    /** true : la page s'ouvre dans l'application ; sinon elle part dans le navigateur. */
    private fun allowInApp(uri: Uri, newWindow: Boolean): Boolean {
        when (uri.scheme?.lowercase()) {
            "looty" -> { handleIntent(Intent(Intent.ACTION_VIEW, uri)); return false }
            "about", "blob", "data" -> return !newWindow
            "http", "https" -> {
                if (sameOrigin(uri)) {
                    if (!uri.path.orEmpty().startsWith("/api/auth/")) signingInUntil = 0
                    return true
                }
                if (!newWindow && System.currentTimeMillis() < signingInUntil) return true
            }
        }
        openExternal(uri)
        return false
    }

    private fun openExternal(uri: Uri) {
        try {
            startActivity(Intent(Intent.ACTION_VIEW, uri).addCategory(Intent.CATEGORY_BROWSABLE))
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(this, R.string.no_browser, Toast.LENGTH_SHORT).show()
        }
    }

    /** Liens looty://w/… et retour de connexion looty://sso. Renvoie true si une page a été chargée. */
    private fun handleIntent(intent: Intent?): Boolean {
        intent?.getStringExtra(EXTRA_PATH)?.let { p ->
            if (p.startsWith("/") && !p.startsWith("//")) { web.loadUrl(server + p); return true }
        }
        val uri = intent?.data ?: return false
        if (uri.scheme != "looty") return false
        if (uri.host == "sso") {
            val code = uri.getQueryParameter("code") ?: return false
            val state = uri.getQueryParameter("state") ?: return false
            web.loadUrl("$server/__looty/sso?code=${Uri.encode(code)}&state=${Uri.encode(state)}")
            return true
        }
        val path = "/" + (uri.host ?: "") + (uri.encodedPath ?: "") + (uri.encodedQuery?.let { "?$it" } ?: "") + (uri.encodedFragment?.let { "#$it" } ?: "")
        if (!path.startsWith("/w/")) return false
        web.loadUrl(server + path)
        return true
    }

    // ---------------------------------------------------------------------
    // Connexion par le navigateur (RFC 8252)
    // ---------------------------------------------------------------------

    private fun html(s: String) = WebResourceResponse("text/html", "utf-8", ByteArrayInputStream(s.toByteArray())).apply {
        responseHeaders = mapOf("Cache-Control" to "no-store")
    }

    private fun intercept(request: WebResourceRequest): WebResourceResponse? {
        val url = request.url
        if (!sameOrigin(url) || request.method != "GET") return null
        val path = url.path.orEmpty()
        return when {
            path.startsWith("/api/auth/login/") -> {
                if (!androidSso) {
                    signingInUntil = System.currentTimeMillis() + 10 * 60_000
                    return null
                }
                val provider = path.removePrefix("/api/auth/login/")
                val redirect = url.getQueryParameter("redirect")?.takeIf { it.startsWith("/") && !it.startsWith("//") } ?: "/"
                val state = randomToken()
                val verifier = randomToken()
                val challenge = Base64.encodeToString(MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray()), Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)
                val browserUrl = "$server/api/auth/desktop/start?provider=${Uri.encode(provider)}&app=android&state=$state&challenge=$challenge"
                Config.setPendingSso(this, Config.PendingSso(state, verifier, redirect, browserUrl, System.currentTimeMillis()))
                main.post { openBrowser(browserUrl) }
                WebResourceResponse("application/json", "utf-8", ByteArrayInputStream("""{"url":"/__looty/sso-wait"}""".toByteArray()))
            }
            path == "/__looty/sso-wait" -> html(Pages.ssoWait())
            path == "/__looty/sso" -> {
                val pending = Config.pendingSso(this)
                val code = url.getQueryParameter("code")
                if (pending == null || code == null || url.getQueryParameter("state") != pending.state) return html(Pages.ssoExpired())
                Config.setPendingSso(this, null)
                html(Pages.ssoReturn(code, pending.verifier, pending.redirect))
            }
            else -> null
        }
    }

    private fun openBrowser(url: String) {
        val uri = Uri.parse(url)
        try {
            CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(this, uri)
        } catch (_: ActivityNotFoundException) {
            openExternal(uri)
        }
    }

    private fun randomToken(): String {
        val b = ByteArray(32)
        SecureRandom().nextBytes(b)
        return Base64.encodeToString(b, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)
    }

    // ---------------------------------------------------------------------
    // Messages de la page
    // ---------------------------------------------------------------------

    private fun onPageMessage(raw: String) {
        val m = runCatching { JSONObject(raw) }.getOrNull() ?: return
        when (m.optString("type")) {
            "reopenSso" -> Config.pendingSso(this)?.let { openBrowser(it.browserUrl) }
            "retry" -> web.loadUrl(failedUrl ?: "$server/")
            "changeServer" -> startActivity(Intent(this, SetupActivity::class.java))
            "close" -> finish()
            "back" -> if (web.canGoBack()) web.goBack() else finish()
            "theme" -> applyTheme(m.optBoolean("dark"))
            "moodleLogin" -> moodleLogin.launch(Intent(this, MoodleLoginActivity::class.java).putExtra(MoodleLoginActivity.EXTRA_URL, m.optString("url")))
            "file" -> saveFile(m.optString("name", "download"), m.optString("mime"), m.optString("data"))
        }
    }

    // ---------------------------------------------------------------------
    // Téléchargements
    // ---------------------------------------------------------------------

    private fun download(url: String, userAgent: String, contentDisposition: String?, mimeType: String?) {
        val name = URLUtil.guessFileName(url, contentDisposition, mimeType)
        try {
            val req = DownloadManager.Request(Uri.parse(url))
                .addRequestHeader("Cookie", CookieManager.getInstance().getCookie(url) ?: "")
                .addRequestHeader("User-Agent", userAgent)
                .setMimeType(mimeType)
                .setTitle(name)
                .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
            if (Build.VERSION.SDK_INT >= 29) req.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
            else req.setDestinationInExternalFilesDir(this, Environment.DIRECTORY_DOWNLOADS, name)
            getSystemService(DownloadManager::class.java).enqueue(req)
            Toast.makeText(this, getString(R.string.download_started, name), Toast.LENGTH_SHORT).show()
        } catch (_: Exception) {
            Toast.makeText(this, R.string.download_failed, Toast.LENGTH_SHORT).show()
        }
    }

    /** Fichier produit par la page (blob:), reçu en base64. */
    private fun saveFile(name: String, mime: String, b64: String) {
        thread {
            val ok = runCatching {
                val bytes = Base64.decode(b64, Base64.DEFAULT)
                val safe = name.replace(Regex("[\\\\/:*?\"<>|]"), "_").ifBlank { "download" }
                if (Build.VERSION.SDK_INT >= 29) {
                    val values = ContentValues().apply {
                        put(MediaStore.Downloads.DISPLAY_NAME, safe)
                        put(MediaStore.Downloads.MIME_TYPE, mime.ifBlank { "application/octet-stream" })
                    }
                    val uri = contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values) ?: error("insert")
                    contentResolver.openOutputStream(uri)!!.use { it.write(bytes) }
                } else {
                    File(getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS), safe).writeBytes(bytes)
                }
                safe
            }
            main.post {
                ok.onSuccess { Toast.makeText(this, getString(R.string.download_saved, it), Toast.LENGTH_SHORT).show() }
                    .onFailure { Toast.makeText(this, R.string.download_failed, Toast.LENGTH_SHORT).show() }
            }
        }
    }
}

/**
 * Injecté dans les pages du serveur : thème clair/sombre de la page (couleur des barres
 * système) et fichiers générés dans la page (liens blob: avec « download »).
 */
private const val PAGE_SCRIPT = """
(() => {
  // Page affichée dans l'application native : l'interface masque sa propre navigation.
  window.__lootyNative = true;
  document.documentElement.classList.add('native-embed');
  if (!window.LootyApp) return;
  const post = (m) => LootyApp.postMessage(JSON.stringify(m));
  const theme = () => post({ type: 'theme', dark: document.documentElement.classList.contains('dark') });
  new MutationObserver(theme).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
  document.addEventListener('DOMContentLoaded', theme);
  window.__lootySave = (href, name) => fetch(href).then((r) => r.blob()).then((b) => {
    const fr = new FileReader();
    fr.onload = () => post({ type: 'file', name: name || 'download', mime: b.type, data: String(fr.result).split(',')[1] || '' });
    fr.readAsDataURL(b);
  });
  const grab = (a) => {
    if (a && a.hasAttribute('download') && /^(blob|data):/.test(a.href)) { __lootySave(a.href, a.getAttribute('download')); return true; }
    return false;
  };
  const click = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () { if (!grab(this)) click.call(this); };
  document.addEventListener('click', (e) => {
    const a = e.target && e.target.closest && e.target.closest('a[download]');
    if (a && grab(a)) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
})();
"""
