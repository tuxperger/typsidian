package dev.typsidian.capture

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.MimeTypeMap
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject
import java.io.File
import java.util.Calendar
import java.util.UUID

@InvokeArg
class IdArgs {
  lateinit var id: String
}

@InvokeArg
class FileArgs {
  lateinit var file: String
}

/**
 * Очередь захватов «Поделиться» и ярлыки приложения (фича 022).
 *
 * Принимает `ACTION_SEND` (`text/plain`, картинки `image/…`) и `ACTION_VIEW` с адресом
 * `typsidian://today` и `typsidian://capture`. Захват сразу превращается в файл
 * `filesDir/captures/pending/<uuid>.json` — ровно той формы, что `Capture` ядра
 * (`packages/core/src/capture`), — и лежит там, пока окно не вызовет `resolve`
 * после записи в vault (FR-005: отказ записи ничего не теряет). Картинку плагин
 * копирует в `filesDir/captures/files/` немедленно: разрешение на чужой `content://`
 * живёт только пока жива активность, получившая намерение.
 *
 * Записывать в vault плагин не умеет и не должен: это делает ядро по команде
 * окна (принцип I).
 */
@TauriPlugin
class CapturePlugin(private val activity: Activity) : Plugin(activity) {
  /** Окно открыто намерением `ACTION_SEND`: из него можно вернуться назад. */
  private var fromSend = false

  /** Ярлык, пришедший раньше, чем окно подписалось на `shortcut` (холодный старт). */
  private var pendingShortcut: String? = null

  private val root: File get() = File(activity.filesDir, "captures")
  private val queueDir: File get() = File(root, "pending").apply { mkdirs() }
  private val filesDir: File get() = File(root, "files").apply { mkdirs() }

  /** Холодный старт: `onNewIntent` для первого намерения не вызывается. */
  override fun load(webView: WebView) {
    super.load(webView)
    activity.intent?.let { handle(it) }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    handle(intent)
  }

  /**
   * Подписка на событие. Ярлык холодного старта приходит до того, как веб-часть
   * успела подписаться, поэтому «Сегодня» и «Во входящие» ждут здесь.
   */
  @Command
  override fun registerListener(invoke: Invoke) {
    super.registerListener(invoke)
    if (invoke.getArgs().optString("event") == "shortcut") {
      pendingShortcut?.let {
        pendingShortcut = null
        trigger("shortcut", JSObject().put("action", it))
      }
    }
  }

  /** Ждущие захваты в порядке поступления; нечитаемые файлы пропускаются, но остаются. */
  @Command
  fun pending(invoke: Invoke) {
    val captures = JSArray()
    val files = queueDir.listFiles { f -> f.isFile && f.name.endsWith(".json") }.orEmpty()
    for (file in files.sortedWith(compareBy({ it.lastModified() }, { it.name }))) {
      try {
        captures.put(JSONObject(file.readText()))
      } catch (_: Exception) {
        // Битая запись не должна прятать остальные.
      }
    }
    invoke.resolve(JSObject().put("captures", captures))
  }

  /** Убирает запись и файл её картинки. Неизвестный `id` — не ошибка. */
  @Command
  fun resolve(invoke: Invoke) {
    val id = invoke.parseArgs(IdArgs::class.java).id
    if (!isSafeName(id)) {
      invoke.reject("недопустимый идентификатор записи")
      return
    }
    val record = File(queueDir, "$id.json")
    try {
      if (record.exists()) {
        val file = JSONObject(record.readText()).optJSONObject("content")?.optString("file")
        if (!file.isNullOrEmpty() && isSafeName(file)) File(filesDir, file).delete()
      }
    } catch (_: Exception) {
      // Запись нечитаема: файл картинки не найти, удаляем хотя бы саму запись.
    }
    record.delete()
    invoke.resolve()
  }

  /** Назад в приложение-источник — только если окно открыто из «Поделиться». */
  @Command
  fun returnToSource(invoke: Invoke) {
    if (fromSend) {
      fromSend = false
      activity.runOnUiThread { activity.moveTaskToBack(true) }
    }
    invoke.resolve()
  }

  /** Байты картинки захвата, base64 одной строкой: `{ data }`. */
  @Command
  fun image(invoke: Invoke) {
    val name = invoke.parseArgs(FileArgs::class.java).file
    val file = File(filesDir, name)
    if (!isSafeName(name) || !file.isFile) {
      invoke.reject("картинки нет в очереди")
      return
    }
    invoke.resolve(JSObject().put("data", Base64.encodeToString(file.readBytes(), Base64.NO_WRAP)))
  }

  /** Разбирает намерение; каждое обрабатывается один раз (см. [HANDLED]). */
  private fun handle(intent: Intent) {
    if (intent.getBooleanExtra(HANDLED, false)) return
    when (intent.action) {
      Intent.ACTION_SEND -> {
        intent.putExtra(HANDLED, true)
        fromSend = true
        if (receive(intent)) trigger("shared", JSObject())
      }
      Intent.ACTION_VIEW -> {
        val uri = intent.data ?: return
        if (uri.scheme != "typsidian") return
        val action = uri.host
        if (action != "today" && action != "capture") return
        intent.putExtra(HANDLED, true)
        fromSend = false
        if (hasListener("shortcut")) {
          trigger("shortcut", JSObject().put("action", action))
        } else {
          pendingShortcut = action
        }
      }
    }
  }

  /** Кладёт захват из `ACTION_SEND` в очередь. `false` — захватывать нечего. */
  private fun receive(intent: Intent): Boolean {
    val id = UUID.randomUUID().toString()
    val content = if (intent.type?.startsWith("image/") == true) {
      imageContent(intent, id)
    } else {
      textContent(intent)
    } ?: return false

    val now = Calendar.getInstance()
    val capture = JSONObject()
      .put("id", id)
      .put("content", content)
      .put("note", JSONObject.NULL)
      .put(
        "at",
        JSONObject()
          .put("year", now.get(Calendar.YEAR))
          .put("month", now.get(Calendar.MONTH) + 1)
          .put("day", now.get(Calendar.DAY_OF_MONTH))
          .put("hour", now.get(Calendar.HOUR_OF_DAY))
          .put("minute", now.get(Calendar.MINUTE)),
      )
      .put("target", JSONObject().put("kind", "Inbox"))

    // Запись появляется целиком или не появляется: читатель не должен увидеть
    // половину файла.
    val tmp = File(queueDir, "$id.tmp")
    tmp.writeText(capture.toString())
    return tmp.renameTo(File(queueDir, "$id.json"))
  }

  /** Текст или ссылка: ссылка — если `EXTRA_TEXT` целиком адрес. */
  private fun textContent(intent: Intent): JSONObject? {
    val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.trim().orEmpty()
    if (text.isEmpty()) return null
    if (URL.matches(text)) {
      val content = JSONObject().put("kind", "Link").put("url", text)
      val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)?.trim().orEmpty()
      if (subject.isNotEmpty()) content.put("title", subject)
      return content
    }
    return JSONObject().put("kind", "Text").put("text", text)
  }

  /** Картинка: копия в `captures/files/<id>.<ext>`, пока есть право на её адрес. */
  private fun imageContent(intent: Intent, id: String): JSONObject? {
    val uri = stream(intent) ?: return null
    val mime = activity.contentResolver.getType(uri) ?: intent.type ?: return null
    val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: "bin"
    val file = "$id.$ext"
    try {
      activity.contentResolver.openInputStream(uri)?.use { input ->
        File(filesDir, file).outputStream().use { input.copyTo(it) }
      } ?: return null
    } catch (_: Exception) {
      File(filesDir, file).delete()
      return null
    }
    return JSONObject().put("kind", "Image").put("file", file).put("mime", mime).put("name", displayName(uri) ?: file)
  }

  private fun displayName(uri: Uri): String? =
    try {
      activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
        if (it.moveToFirst()) it.getString(0)?.takeIf { name -> name.isNotBlank() } else null
      }
    } catch (_: Exception) {
      null
    }

  @Suppress("DEPRECATION")
  private fun stream(intent: Intent): Uri? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
    } else {
      intent.getParcelableExtra(Intent.EXTRA_STREAM)
    }

  /** Имя без разделителей пути: идентификатор или имя файла очереди, не путь. */
  private fun isSafeName(name: String): Boolean =
    name.isNotEmpty() && !name.contains('/') && !name.contains('\\') && !name.startsWith(".")

  private companion object {
    /** Метка обработанного намерения: повторная доставка не создаёт вторую запись. */
    const val HANDLED = "dev.typsidian.capture.HANDLED"

    /** `EXTRA_TEXT` целиком — адрес. */
    val URL = Regex("^https?://\\S+$", RegexOption.IGNORE_CASE)
  }
}
