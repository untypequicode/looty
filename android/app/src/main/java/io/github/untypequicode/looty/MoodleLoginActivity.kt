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
 */
class MoodleLoginActivity : ComponentActivity() {
    companion object {
        const val EXTRA_URL = "url"
        const val EXTRA_LINK = "link"
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
            contentDescription = getString(R.string.cancel)
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
                    "moodlemobile" -> { done(uri.toString()); true }
                    else -> true
                }
            }

            override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) { progress.visibility = android.view.View.VISIBLE }

            override fun onPageFinished(view: WebView, url: String?) {
                progress.visibility = android.view.View.INVISIBLE
                title.text = view.title?.takeIf { it.isNotBlank() } ?: Uri.parse(url ?: "").host
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
