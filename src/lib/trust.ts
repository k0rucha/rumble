// 送信元の検証結果を、画面に出す言葉にする。
// 判定そのものはバックエンド (crates/rumble-core/src/authenticity) が行う。

import type { IconName } from "../components/Icon.svelte";
import type { CheckStatus, DmarcPolicy, SenderAuth, TrustLevel } from "./types";

export const TRUST_LABELS: Record<TrustLevel, string> = {
  verified: "送信元を確認済み",
  reported: "受信サーバーが確認",
  partial: "差出人とは別ドメインの認証のみ",
  failed: "送信元の検証に失敗",
  unknown: "送信元を確認できません",
};

export const TRUST_ICONS: Record<TrustLevel, IconName> = {
  verified: "shieldCheck",
  reported: "shieldCheck",
  partial: "shieldAlert",
  failed: "shieldX",
  unknown: "shieldQuestion",
};

/** 判定の意味を、専門用語を使わずに 1〜2 文で説明する */
export function trustExplanation(auth: SenderAuth): string {
  const domain = auth.fromDomain ?? "差出人のドメイン";
  switch (auth.level) {
    case "verified":
      return `${domain} の電子署名を rumble が検算し、このメールが ${domain} から出され、途中で書き換えられていないことを確かめました。`;
    case "reported":
      return `受信サーバー (${auth.reported?.verifier ?? "不明"}) が、${domain} からのメールだと確認したと記録しています。rumble 自身では検算できていません。`;
    case "partial":
      return `署名や送信経路の検査は通っていますが、差出人欄の ${domain} についてのものではありません。配信サービス経由のメールでよくある状態ですが、差出人欄そのものは裏付けられていません。`;
    case "failed":
      return "なりすまし、または配送の途中で内容が書き換えられた可能性があります。リンクや添付ファイルを開く前に、差出人をよく確かめてください。";
    case "unknown":
      return "電子署名も、受信サーバーの検査記録もありません。差出人欄は送る側が自由に書けるため、本当の送信元は分かりません。";
  }
}

export const STATUS_LABELS: Record<CheckStatus, string> = {
  pass: "合格",
  fail: "不合格",
  inconclusive: "判定できず",
  none: "対象なし",
};

export const POLICY_LABELS: Record<DmarcPolicy, string> = {
  reject: "reject — 検証に失敗したメールは受け取りを拒否させる",
  quarantine: "quarantine — 検証に失敗したメールは迷惑メール扱いにさせる",
  none: "none — 検証に失敗しても何もしない (なりすましを止めていない)",
};
