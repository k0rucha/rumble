// 「いちばん手前のものを閉じる」操作 (Android の戻るボタン) のための登録簿。
//
// ダイアログのように手前に重なる部品は、表示されている間だけ自分の閉じ方を登録する。
// 戻るボタンが押されたら、最後に登録されたもの (= いちばん手前) を閉じる。

const dismissers: (() => void)[] = [];

/** 閉じ方を登録する。戻り値を呼ぶと登録を外す (部品が消えるときに呼ぶ) */
export function dismissOnBack(dismiss: () => void): () => void {
  dismissers.push(dismiss);
  return () => {
    const index = dismissers.lastIndexOf(dismiss);
    if (index !== -1) dismissers.splice(index, 1);
  };
}

/** いちばん手前の部品を閉じる。閉じるものが無ければ false */
export function dismissTopmost(): boolean {
  const dismiss = dismissers.at(-1);
  dismiss?.();
  return dismiss !== undefined;
}
