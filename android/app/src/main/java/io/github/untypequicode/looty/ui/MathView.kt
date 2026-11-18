package io.github.untypequicode.looty.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color as AColor
import android.util.LruCache
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.ui.text.withAnnotation
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle

/** HTML autonome rendant une formule avec le KaTeX embarqué (assets/katex), sans réseau. */
internal object MathHtml {
    /** Littéral de chaîne JS sûr à placer dans un <script> (aucun `</script>` ni séparateur de ligne possible). */
    fun jsString(s: String): String = buildString {
        append('"')
        for (c in s) when (c) {
            '\\' -> append("\\\\")
            '"' -> append("\\\"")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '<' -> append("\\u003c")
            '>' -> append("\\u003e")
            '&' -> append("\\u0026")
            '\u2028' -> append("\\u2028")
            '\u2029' -> append("\\u2029")
            else -> append(c)
        }
        append('"')
    }

    fun cssColor(argb: Int): String =
        "rgba(${AColor.red(argb)},${AColor.green(argb)},${AColor.blue(argb)},${AColor.alpha(argb) / 255f})"


    fun page(latex: String, display: Boolean, color: String): String = """<!doctype html><html><head>
<meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="stylesheet" href="katex.min.css"><script src="katex.min.js"></script>
<style>html,body{margin:0;padding:0;background:transparent;color:$color;overflow-y:hidden}
body{display:${if (display) "block" else "inline-block"};font-size:${if (display) "1.15em" else "1em"};overflow-x:${if (display) "auto" else "hidden"}}
#m{display:${if (display) "block" else "inline-block"};padding:2px 0;white-space:nowrap}
.katex-display{margin:.2em 0}.err{font-family:monospace;font-style:italic}</style></head>
<body><div id="m"></div><script>
var el=document.getElementById('m');
try{katex.render(${jsString(latex)},el,{displayMode:$display,throwOnError:false,output:'html',trust:false,strict:'ignore'})}
catch(e){el.className='err';el.textContent=${jsString(latex)}}
function rep(){var r=el.getBoundingClientRect();Looty.size(Math.ceil(r.width),Math.ceil(r.height)+4)}
(document.fonts&&document.fonts.ready?document.fonts.ready:Promise.resolve()).then(function(){requestAnimationFrame(rep)});
</script></body></html>"""
}

internal data class MathSize(val w: Int, val h: Int)

/** Tailles déjà mesurées (px CSS) : évite le saut de mise en page quand la formule revient à l'écran. */
private val sizes = LruCache<String, MathSize>(300)

/** Incrémenté à chaque nouvelle mesure : les textes avec formules en ligne se recomposent avec la vraie taille. */
internal var mathTick by mutableIntStateOf(0)

internal fun sizeKey(latex: String, display: Boolean) = "${if (display) "d" else "i"}|$latex"

internal fun measuredMathSize(latex: String): MathSize? = sizes.get(sizeKey(latex, false))

private class Slot(val web: WebView) {
    var onSize: ((Int, Int) -> Unit)? = null
}

/** Petit réservoir de WebView réutilisées : en créer une coûte cher, une liste de formules en aurait des dizaines. */
private object MathPool {
    private val free = ArrayDeque<Slot>()

    @SuppressLint("SetJavaScriptEnabled")
    fun obtain(ctx: Context): Slot {
        free.removeLastOrNull()?.let { return it }
        val web = WebView(ctx.applicationContext)
        val slot = Slot(web)
        web.setBackgroundColor(AColor.TRANSPARENT)
        web.isVerticalScrollBarEnabled = false
        web.settings.javaScriptEnabled = true // KaTeX embarqué uniquement ; aucune URL distante n'est chargée.
        web.settings.allowFileAccess = true // file:///android_asset
        web.settings.allowContentAccess = false
        web.settings.blockNetworkLoads = true
        web.addJavascriptInterface(object {
            @JavascriptInterface fun size(w: Int, h: Int) { web.post { slot.onSize?.invoke(w, h) } }
        }, "Looty")
        return slot
    }

    fun release(slot: Slot) {
        slot.onSize = null
        (slot.web.parent as? ViewGroup)?.removeView(slot.web)
        slot.web.loadUrl("about:blank")
        if (free.size < 4) free.addLast(slot) else slot.web.destroy()
    }
}

/**
 * Formule LaTeX rendue avec KaTeX (hors ligne, comme sur le web).
 * `display` = formule centrée en bloc ; sinon formule en ligne. Réutilisable (lecteur, chat IA, révisions).
 */
@Composable
fun MathView(latex: String, display: Boolean, modifier: Modifier = Modifier, color: Color = LocalContentColor.current) {
    if (latex.isBlank()) return
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val key = "${if (display) "d" else "i"}|${color.toArgb()}|$latex"
    var measured by remember(key) { mutableStateOf(sizes.get(sizeKey(latex, display))) }
    val css = remember(color) { MathHtml.cssColor(color.toArgb()) }
    val dp = { px: Int -> with(density) { px.toDp() } }
    val sized = measured?.let {
        if (display) modifier.fillMaxWidth().height(dp(it.h)) else modifier.size(dp(it.w), dp(it.h))
    } ?: if (display) modifier.fillMaxWidth().height(Dp(40f)) else modifier.size(Dp(24f), Dp(24f))
    AndroidView(
        modifier = sized,
        factory = { c -> FrameHolder(c) },
        update = { holder ->
            if (holder.key == key) return@update
            holder.key = key
            holder.slot?.let { MathPool.release(it) }
            val slot = MathPool.obtain(ctx)
            holder.slot = slot
            slot.onSize = { w, h ->
                val s = MathSize(w, h)
                sizes.put(sizeKey(latex, display), s); mathTick++
                measured = s
            }
            (slot.web.parent as? ViewGroup)?.removeView(slot.web)
            holder.addView(slot.web, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
            slot.web.loadDataWithBaseURL("file:///android_asset/katex/", MathHtml.page(latex, display, css), "text/html", "utf-8", null)
        },
        onRelease = { holder -> holder.slot?.let { MathPool.release(it) }; holder.slot = null; holder.key = null },
    )
}

private class FrameHolder(ctx: Context) : android.widget.FrameLayout(ctx) {
    var key: String? = null
    var slot: Slot? = null
}

/** Repli textuel (aperçus RichDoc, notifications) : la source LaTeX en monospace italique. */
@Composable
fun MathFallbackText(latex: String) {
    Text(latex, fontFamily = FontFamily.Monospace, fontStyle = FontStyle.Italic, style = MaterialTheme.typography.bodyMedium)
}

internal const val MATH_TAG = "looty.math"

/** Ajoute une formule en ligne : un emplacement dans le texte, rempli par [MathText] (repli : la source LaTeX). */
internal fun AnnotatedString.Builder.appendInlineMath(latex: String) {
    withAnnotation(MATH_TAG, latex) { appendInlineContent(latex, latex) }
}

/** Text qui affiche les formules en ligne ([appendInlineMath]) avec KaTeX, à la taille mesurée (24sp avant mesure). */
@Composable
fun MathText(text: AnnotatedString, modifier: Modifier = Modifier, style: TextStyle = LocalTextStyle.current, fontWeight: FontWeight? = null) {
    val density = LocalDensity.current
    val color = LocalContentColor.current
    val tick = mathTick // relit la mesure quand une formule vient d'être mesurée
    if (tick < 0) return
    val formulas = text.getStringAnnotations(MATH_TAG, 0, text.length).map { it.item }.distinct()
    val content = if (formulas.isEmpty()) emptyMap() else formulas.associateWith { latex ->
        val m = measuredMathSize(latex)
        val (w, h) = if (m == null) 24.sp to 24.sp else with(density) { m.w.toDp().toSp() to m.h.toDp().toSp() }
        InlineTextContent(Placeholder(w, h, PlaceholderVerticalAlign.TextCenter)) { MathView(latex, display = false, color = color) }
    }
    Text(text, modifier = modifier, style = style, fontWeight = fontWeight, inlineContent = content)
}
