package dev.typsidian.mobile

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

/**
 * Точка входа приложения.
 *
 * Отличается от сгенерированной `tauri android init` ровно одним: запрашивает
 * доступ к общей памяти устройства. Vault живёт в ней и обязан переживать
 * удаление приложения (принцип VI), а без этого разрешения обзор папок не
 * прочитает ни одного каталога и ответит `AccessDenied`.
 *
 * ВНИМАНИЕ: файл сгенерирован, и повторный `cargo tauri android init` перезапишет
 * его вместе с этой правкой. Проверять после каждой такой перегенерации.
 */
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestStorageAccess()
  }

  /**
   * Просит доступ к общей памяти — по-разному на двух эпохах Android.
   *
   * С API 30 диалога не существует: MANAGE_EXTERNAL_STORAGE выдаётся только
   * вручную, на отдельном экране системных настроек, и всё, что может сделать
   * приложение, — открыть этот экран. До API 29 включительно достаточно обычной
   * пары READ/WRITE, которую система спрашивает диалогом.
   *
   * Отказ здесь не обрабатывается и приложение не останавливает: запрос
   * повторяется при каждом запуске, а попытка открыть vault без разрешения
   * вернётся в интерфейс отказом `PermissionDenied` — тем же, которым она
   * возвращается на web.
   */
  private fun requestStorageAccess() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (Environment.isExternalStorageManager()) return
      // Экран настроек именно этого приложения. На прошивках, где его нет,
      // остаётся общий список — он ведёт туда же, только в два шага.
      val forThisApp = Intent(
        Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
        Uri.parse("package:$packageName"),
      )
      try {
        startActivity(forThisApp)
      } catch (_: android.content.ActivityNotFoundException) {
        try {
          startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
        } catch (_: android.content.ActivityNotFoundException) {
          // Ни того, ни другого экрана на устройстве нет. Молча: сказать об
          // этом — работа интерфейса, и он это сделает при открытии vault.
        }
      }
      return
    }

    val missing = arrayOf(
      Manifest.permission.READ_EXTERNAL_STORAGE,
      Manifest.permission.WRITE_EXTERNAL_STORAGE,
    ).filter {
      ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }
    if (missing.isNotEmpty()) {
      ActivityCompat.requestPermissions(this, missing.toTypedArray(), STORAGE_REQUEST)
    }
  }

  private companion object {
    /** Код запроса разрешений; ответ не разбирается — см. `requestStorageAccess`. */
    const val STORAGE_REQUEST = 1
  }
}
