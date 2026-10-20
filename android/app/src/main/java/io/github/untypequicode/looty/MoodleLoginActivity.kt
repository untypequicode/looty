package io.github.untypequicode.looty

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Connexion à Moodle par le portail officiel (CAS, SSO…) dans une WebView dédiée.
 *
 * Moodle termine la connexion en redirigeant vers `moodlemobile://token=…`, le lien de
 * sa propre application : on l'intercepte ici et on le renvoie à Looty, qui l'échange
 * contre une clé d'accès. Le mot de passe ne passe jamais par Looty.
 *
 * Mode « site » (`EXTRA_BROWSE`) : le site Moodle dans l'application, ouvert déjà connecté
 * par le lien de connexion automatique ; la session (cookies) reste pour les fois suivantes.
 */
class MoodleLoginActivity : ComponentActivity() {
    companion object {
        const val EXTRA_URL = "url"
        const val EXTRA_LINK = "link"
        const val EXTRA_BROWSE = "browse"

        /** Ouvre une page du site Moodle dans l'application. */
        fun browse(ctx: android.content.Context, url: String) {
            ctx.startActivity(Intent(ctx, MoodleLoginActivity::class.java).putExtra(EXTRA_URL, url).putExtra(EXTRA_BROWSE, true).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }

    private lateinit var web: WebView
    private lateinit var progress: ProgressBar
    private lateinit var title: TextView

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val start = intent.getStringExtra(EXTRA_URL)?.let(Uri::parse)
        if (start == null || start.scheme?.lowercase() !in setOf("http", "https")) {
            finish()
            return
        }
        val browsing = intent.getBooleanExtra(EXTRA_BROWSE, false)
        enableEdgeToEdge()
        val dp = resources.displayMetrics.density
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val close = ImageButton(this).apply {
            setImageResource(R.drawable.ic_close)
            setBackgroundColor(0)
            contentDescription = getString(if (browsing) R.string.close else R.string.cancel)
            setOnClickListener { finish() }
        }
        title = TextView(this).apply {
            textSize = 16f
            maxLines = 1
            text = start.host
        }
        bar.addView(close, LinearLayout.LayoutParams((48 * dp).toInt(), (48 * dp).toInt()))
        bar.addView(title, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        progress = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply { isIndeterminate = true }
        web = WebView(this)
        root.addView(bar, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        root.addView(progress, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, (3 * dp).toInt()))
        root.addView(web, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        setContentView(FrameLayout(this).also { it.addView(root) })
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            WindowInsetsCompat.CONSUMED
        }

        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            allowFileAccess = false
            allowContentAccess = false
        }
        CookieManager.getInstance().setAcceptCookie(true)
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val uri = request.url
                return when (uri.scheme?.lowercase()) {
                    "http", "https" -> false
                    "moodlemobile" -> { if (!browsing) done(uri.toString()); true }
                    // mailto:, tel:… : application du téléphone (mode site seulement).
                    else -> { if (browsing) runCatching { startActivity(Intent(Intent.ACTION_VIEW, uri)) }; true }
                }
            }

            override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) { progress.visibility = android.view.View.VISIBLE }

            override fun onPageFinished(view: WebView, url: String?) {
                progress.visibility = android.view.View.INVISIBLE
                title.text = view.title?.takeIf { it.isNotBlank() } ?: Uri.parse(url ?: "").host
            }
        }
        // Fichiers du site : gestionnaire de téléchargements, avec la session Moodle.
        if (browsing) web.setDownloadListener { url, userAgent, disposition, mime, _ ->
            runCatching {
                val name = android.webkit.URLUtil.guessFileName(url, disposition, mime)
                val req = android.app.DownloadManager.Request(Uri.parse(url))
                    .addRequestHeader("Cookie", CookieManager.getInstance().getCookie(url) ?: "")
                    .addRequestHeader("User-Agent", userAgent)
                    .setMimeType(mime)
                    .setTitle(name)
                    .setNotificationVisibility(android.app.DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                    .setDestinationInExternalPublicDir(android.os.Environment.DIRECTORY_DOWNLOADS, name)
                (getSystemService(DOWNLOAD_SERVICE) as android.app.DownloadManager).enqueue(req)
                android.widget.Toast.makeText(this, name, android.widget.Toast.LENGTH_SHORT).show()
            }
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (web.canGoBack()) web.goBack() else finish()
            }
        })
        web.loadUrl(start.toString())
    }

    private fun done(link: String) {
        setResult(RESULT_OK, Intent().putExtra(EXTRA_LINK, link))
        finish()
    }

    override fun onDestroy() {
        if (::web.isInitialized) web.destroy()
        super.onDestroy()
    }
}
