package io.github.untypequicode.looty.api

import android.content.Context
import android.webkit.CookieManager
import io.github.untypequicode.looty.BuildConfig
import io.github.untypequicode.looty.Config
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.Cookie
import okhttp3.CookieJar
import okhttp3.HttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.MultipartBody
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/** Erreur de l'API Looty (`{ "error": "code" }`). */
class ApiException(val status: Int, val code: String) : Exception("$status $code")

/**
 * Cookies partagés avec la WebView : une seule connexion pour les écrans natifs et les
 * écrans web (la session est posée par la connexion SSO dans la WebView).
 */
private object WebViewCookies : CookieJar {
    override fun loadForRequest(url: HttpUrl): List<Cookie> {
        val header = CookieManager.getInstance().getCookie(url.toString()) ?: return emptyList()
        return header.split(";").mapNotNull { Cookie.parse(url, it.trim()) }
    }

    override fun saveFromResponse(url: HttpUrl, cookies: List<Cookie>) {
        val cm = CookieManager.getInstance()
        cookies.forEach { cm.setCookie(url.toString(), it.toString()) }
        cm.flush()
    }
}

/** Client de l'API du serveur choisi. */
class Api(private val ctx: Context) {
    val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true }

    private val client = OkHttpClient.Builder()
        .cookieJar(WebViewCookies)
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .addInterceptor { chain ->
            chain.proceed(chain.request().newBuilder().header("User-Agent", "LootyAndroid/${BuildConfig.VERSION_NAME} (native)").build())
        }
        .build()

    val server: String get() = Config.server(ctx) ?: throw ApiException(0, "no_server")

    suspend fun raw(method: String, path: String, body: String? = null): String = withContext(Dispatchers.IO) {
        val rb = body?.toRequestBody("application/json".toMediaType()) ?: if (method == "GET" || method == "DELETE") null else "".toRequestBody()
        val req = Request.Builder().url(server + path).method(method, rb).build()
        client.newCall(req).execute().use { res ->
            val text = res.body.string()
            if (!res.isSuccessful) {
                val code = runCatching { json.parseToJsonElement(text).jsonObject["error"]?.jsonPrimitive?.content }.getOrNull()
                throw ApiException(res.code, code ?: "http_${res.code}")
            }
            text
        }
    }

    /** Envoie un fichier (champ « file », comme l'interface web). */
    suspend fun upload(path: String, filename: String, mime: String, bytes: ByteArray) = withContext(Dispatchers.IO) {
        val body = MultipartBody.Builder().setType(MultipartBody.FORM)
            .addFormDataPart("file", filename, bytes.toRequestBody(mime.toMediaType()))
            .build()
        client.newCall(Request.Builder().url(server + path).post(body).build()).execute().use { res ->
            if (!res.isSuccessful) throw ApiException(res.code, "http_${res.code}")
        }
    }

    /** Fichier du serveur (image d’un document…), avec la session. */
    suspend fun bytes(path: String): ByteArray = withContext(Dispatchers.IO) {
        client.newCall(Request.Builder().url(server + path).build()).execute().use { res ->
            if (!res.isSuccessful) throw ApiException(res.code, "http_${res.code}")
            res.body.bytes()
        }
    }

    suspend inline fun <reified T> get(path: String): T = json.decodeFromString(raw("GET", path))
    suspend inline fun <reified T> post(path: String, body: JsonElement? = null): T = json.decodeFromString(raw("POST", path, body?.toString()).ifBlank { "null" })
    suspend fun send(method: String, path: String, body: JsonElement? = null) { raw(method, path, body?.toString()) }
}
