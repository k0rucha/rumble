// 表示用の整形。状態を持たない純粋な関数だけを置く。

import type { Address, Mailbox, MailboxRole } from "./types";

const ROLE_LABELS: Record<Exclude<MailboxRole, "other">, string> = {
  inbox: "受信トレイ",
  flagged: "スター付き",
  drafts: "下書き",
  sent: "送信済み",
  archive: "アーカイブ",
  all: "すべてのメール",
  junk: "迷惑メール",
  trash: "ゴミ箱",
};

/** 役割のあるフォルダは、サーバーごとに違う名前ではなく共通の呼び名で見せる */
export function mailboxLabel(mailbox: Mailbox): string {
  return mailbox.role === "other" ? mailbox.name : ROLE_LABELS[mailbox.role];
}

export function addressLabel(address: Address | null | undefined): string {
  if (!address) return "(差出人なし)";
  return address.name || address.email;
}

export function addressFull(address: Address): string {
  return address.name ? `${address.name} <${address.email}>` : address.email;
}

const timeFormat = new Intl.DateTimeFormat("ja-JP", { hour: "2-digit", minute: "2-digit" });
const dayFormat = new Intl.DateTimeFormat("ja-JP", { month: "numeric", day: "numeric" });
const yearFormat = new Intl.DateTimeFormat("ja-JP", {
  year: "numeric",
  month: "numeric",
  day: "numeric",
});
const fullFormat = new Intl.DateTimeFormat("ja-JP", { dateStyle: "long", timeStyle: "short" });

/** 一覧用の短い日付: 今日なら時刻、今年なら月日、それ以外は年月日 */
export function shortDate(unixSeconds: number | null, now = new Date()): string {
  if (unixSeconds === null) return "";
  const date = new Date(unixSeconds * 1000);
  if (date.toDateString() === now.toDateString()) return timeFormat.format(date);
  if (date.getFullYear() === now.getFullYear()) return dayFormat.format(date);
  return yearFormat.format(date);
}

export function fullDate(unixSeconds: number | null): string {
  return unixSeconds === null ? "" : fullFormat.format(new Date(unixSeconds * 1000));
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** 「a@x, b@y; c@z」のような入力をアドレスの配列にする */
export function splitRecipients(input: string): string[] {
  return input
    .split(/[,;\n]/)
    .map((part) => part.trim())
    .filter(Boolean);
}

/** 返信・転送で元の本文を引用する */
export function quote(text: string | null): string {
  return (text ?? "")
    .trimEnd()
    .split("\n")
    .map((line) => `> ${line}`.trimEnd())
    .join("\n");
}

export function withPrefix(subject: string, prefix: "Re" | "Fwd"): string {
  return new RegExp(`^${prefix}:`, "i").test(subject.trim()) ? subject : `${prefix}: ${subject}`;
}
