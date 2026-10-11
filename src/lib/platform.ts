// 実行環境による違い。画面幅ではなく「何の上で動いているか」で決まるものだけを置く
// (幅による見た目の切り替えは CSS のメディアクエリで行う)。

/** Android / iOS のアプリとして動いているか */
export const isMobile = /Android|iPhone|iPad/i.test(navigator.userAgent);
