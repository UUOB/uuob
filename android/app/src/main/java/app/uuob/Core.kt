package app.uuob

import android.content.Context
import app.uuob.core.UuobException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

/**
 * UUOB's core (Rust, crates/uuob-app), as the screens and the service use it.
 * The functions in app.uuob.core are written by UniFFI; methods and answers
 * are JSON, the same that apps and AI clients use.
 */
object Core {
  /** Lives as long as the process: the node does too. */
  val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

  private val changes = MutableSharedFlow<JSONObject>(extraBufferCapacity = 64)
  /** What changed, e.g. {"type": "device_joined", "name": "…"}. "lagged" means: reload everything. */
  val events: SharedFlow<JSONObject> = changes
  @Volatile private var following = false
  /** Done once UUOB has started (or failed to): calls wait for it. */
  private val ready = CompletableDeferred<Unit>()

  /** Starts UUOB (the screen or the service, whichever comes first) and starts hearing its changes. */
  suspend fun start(ctx: Context) {
    try {
      app.uuob.core.start(ctx.dataDir.absolutePath)
    } catch (e: UuobException.Failed) {
      throw Failed(e.code, e.detail)
    } finally {
      ready.complete(Unit)
    }
    synchronized(this) {
      if (following) return
      following = true
    }
    scope.launch {
      while (true) {
        val e = JSONObject(app.uuob.core.nextEvent())
        changes.emit(e)
        if (e.optString("type") == "closed") delay(1000)
      }
    }
  }

  /** A method that failed; `message` is for the user. */
  class Failed(val code: String, message: String) : Exception(message)

  /** Runs `method`; its answer is a JSONObject, JSONArray, String, Number, Boolean or null. */
  suspend fun call(method: String, params: JSONObject? = null): Any? {
    ready.await()
    val text = try {
      app.uuob.core.call(method, params?.toString() ?: "")
    } catch (e: UuobException.Failed) {
      throw Failed(e.code, e.detail)
    }
    return JSONTokener(text).nextValue().takeIf { it != JSONObject.NULL }
  }

  suspend fun obj(method: String, params: JSONObject? = null) = call(method, params) as? JSONObject ?: JSONObject()
  suspend fun list(method: String, params: JSONObject? = null) = (call(method, params) as? JSONArray).objects()
}

fun params(vararg pairs: Pair<String, Any?>) = JSONObject().apply { pairs.forEach { (k, v) -> put(k, v ?: JSONObject.NULL) } }

fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).map { getJSONObject(it) }

fun JSONObject.objects(key: String) = optJSONArray(key).objects()

/** A text field, or null when it is missing or null. */
fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key)

/** An object's entries, e.g. the clients by id. */
fun JSONObject.entries(): List<Pair<String, JSONObject>> = keys().asSequence().map { it to getJSONObject(it) }.toList()
