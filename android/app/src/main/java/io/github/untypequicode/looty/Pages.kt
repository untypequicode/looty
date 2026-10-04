package io.github.untypequicode.looty

import org.json.JSONObject
import java.util.Locale

/** Petites pages servies par l'application (attente de connexion, erreur réseau). */
object Pages {
    private val fr get() = Locale.getDefault().language == "fr"
    private fun t(fr: String, en: String) = if (this.fr) fr else en
    private fun esc(s: String) = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

    private fun page(title: String, body: String, script: String = "") = """<!doctype html>
<html lang="${if (fr) "fr" else "en"}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(title)}</title><style>
:root{color-scheme:light dark;--bg:#fcfcfd;--text:#1f2023;--muted:#6b6f76;--accent:#5e6ad2;--border:#e4e5e9}
@media(prefers-color-scheme:dark){:root{--bg:#0b0b0d;--text:#e6e6e6;--muted:#8a8f98;--border:#26272b}}
html,body{margin:0;height:100%;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,sans-serif}
main{min-height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:32px;box-sizing:border-box;text-align:center}
h1{font-size:20px;font-weight:600;margin:0}p{margin:0;color:var(--muted);max-width:340px}
.btn{display:inline-block;padding:11px 20px;border-radius:10px;background:var(--accent);color:#fff;text-decoration:none;border:0;font:inherit;font-weight:500;min-width:200px}
.btn.secondary{background:transparent;color:var(--text);box-shadow:inset 0 0 0 1px var(--border)}.link{color:var(--muted);font-size:14px}
.spin{width:28px;height:28px;border:3px solid var(--border);border-top-color:var(--accent);border-radius:50%;animation:s 0.9s linear infinite}@keyframes s{to{transform:rotate(1turn)}}
</style></head><body><main>$body</main><script>const app=(m)=>window.LootyApp&&LootyApp.postMessage(JSON.stringify(m));$script</script></body></html>"""

    fun ssoWait(): String = page(
        t("Connexion", "Sign in"),
        """<div class="spin"></div><h1>${t("Connexion dans le navigateur", "Continue in your browser")}</h1>
<p>${t("Terminez la connexion dans le navigateur qui vient de s’ouvrir, puis revenez ici.", "Finish signing in in the browser that just opened, then come back here.")}</p>
<button class="btn" onclick="app({type:'reopenSso'})">${t("Rouvrir le navigateur", "Open the browser again")}</button>
<a class="link" href="/login">${t("Annuler", "Cancel")}</a>""",
    )

    /** Retour du navigateur : la page échange le code contre la session de l'application. */
    fun ssoReturn(code: String, verifier: String, redirect: String): String = page(
        t("Connexion", "Sign in"),
        """<div class="spin" id="spin"></div><h1 id="h">${t("Connexion…", "Signing in…")}</h1><p id="p"></p><a class="btn" id="retry" href="/login" style="display:none">${t("Réessayer", "Try again")}</a>""",
        """
fetch('/api/auth/desktop/exchange',{method:'POST',credentials:'include',headers:{'content-type':'application/json'},body:JSON.stringify({code:${JSONObject.quote(code)},verifier:${JSONObject.quote(verifier)}})})
.then(r=>{if(!r.ok)throw r.status;location.replace(${JSONObject.quote(redirect)})})
.catch(()=>{document.getElementById('spin').remove();document.getElementById('h').textContent=${JSONObject.quote(t("Connexion impossible", "Couldn’t sign in"))};
document.getElementById('p').textContent=${JSONObject.quote(t("Ce lien a expiré ou a déjà servi. Relancez la connexion.", "This link has expired or was already used. Please sign in again."))};document.getElementById('retry').style.display=''})""",
    )

    fun ssoExpired(): String = page(
        t("Connexion", "Sign in"),
        """<h1>${t("Connexion expirée", "Sign-in expired")}</h1><p>${t("Relancez la connexion depuis l’application.", "Please start signing in again from the app.")}</p><a class="btn" href="/login">${t("Se connecter", "Sign in")}</a>""",
    )

    fun offline(server: String): String = page(
        "Looty",
        """<h1>${t("Serveur injoignable", "Can’t reach the server")}</h1><p>${esc(server.removePrefix("https://"))}<br>${t("Vérifiez votre connexion.", "Check your connection.")}</p>
<button class="btn" onclick="app({type:'retry'})">${t("Réessayer", "Try again")}</button>
<button class="btn secondary" onclick="app({type:'changeServer'})">${t("Changer de serveur", "Change server")}</button>""",
        "addEventListener('online',()=>app({type:'retry'}))",
    )
}
