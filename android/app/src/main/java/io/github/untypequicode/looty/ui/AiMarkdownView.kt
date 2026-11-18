package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp

private fun AnnotatedString.Builder.appendMd(list: List<MdInline>, codeBg: Color, link: Color) {
    for (x in list) when (x) {
        is MdInline.Text -> append(x.s)
        is MdInline.Bold -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { appendMd(x.c, codeBg, link) }
        is MdInline.Italic -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { appendMd(x.c, codeBg, link) }
        is MdInline.Code -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = codeBg)) { append(x.s) }
        is MdInline.Link -> withLink(LinkAnnotation.Url(x.url, TextLinkStyles(SpanStyle(color = link, textDecoration = TextDecoration.Underline)))) { appendMd(x.c, codeBg, link) }
        is MdInline.Math -> appendInlineMath(x.tex)
    }
}

/** Réponse de l'assistant : Markdown (gras, listes, code, liens) et formules KaTeX (en ligne et en bloc). */
@Composable
fun ChatMarkdown(src: String, modifier: Modifier = Modifier) {
    val blocks = remember(src) { parseMarkdown(src) }
    val codeBg = MaterialTheme.colorScheme.surfaceVariant
    val link = MaterialTheme.colorScheme.primary
    fun annotated(l: List<MdInline>) = buildAnnotatedString { appendMd(l, codeBg, link) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (b in blocks) when (b) {
            is MdBlock.Para -> MathText(annotated(b.inlines), style = MaterialTheme.typography.bodyMedium)
            is MdBlock.Heading -> MathText(annotated(b.inlines), style = if (b.level <= 2) MaterialTheme.typography.titleMedium else MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            is MdBlock.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
                Box(Modifier.width(3.dp).fillMaxHeight().background(MaterialTheme.colorScheme.outline))
                MathText(annotated(b.inlines), Modifier.padding(start = 8.dp), style = MaterialTheme.typography.bodyMedium)
            }
            is MdBlock.Code -> Text(
                b.text, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.fillMaxWidth().background(codeBg, RoundedCornerShape(8.dp)).horizontalScroll(rememberScrollState()).padding(10.dp),
            )
            is MdBlock.Items -> Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                for (it in b.items) Row(Modifier.padding(start = (it.depth * 16).dp)) {
                    Text(it.marker, Modifier.width(22.dp), style = MaterialTheme.typography.bodyMedium)
                    MathText(annotated(it.inlines), style = MaterialTheme.typography.bodyMedium)
                }
            }
            is MdBlock.DisplayMath -> MathView(b.tex, display = true, modifier = Modifier.fillMaxWidth())
            MdBlock.Rule -> HorizontalDivider()
        }
    }
}
