package dev.korucha.rumble

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    keepContentClearOfSystemUi()
  }

  // edge-to-edge (Android 15 以降は強制) では、アプリの画面がステータスバーや
  // ナビゲーションバーの下まで広がる。そのままだと画面上端のボタンが時計や電波表示と
  // 重なって押せないので、システムの表示領域ぶんだけ内側に寄せる。
  // キーボード (IME) も同じ扱いにして、入力欄がキーボードに隠れないようにする。
  private fun keepContentClearOfSystemUi() {
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val occupied = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(occupied.left, occupied.top, occupied.right, occupied.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }
}
