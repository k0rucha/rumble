// よく使われるメールサービスの接続設定。
// アカウント追加時にメールアドレスのドメインから自動で選ぶ。

import type { ServerEndpoint } from "./types";

export interface Provider {
  id: string;
  label: string;
  /** このドメインのアドレスなら自動的に選ぶ */
  domains: string[];
  imap: ServerEndpoint;
  smtp: ServerEndpoint;
  /** パスワード欄の下に出す補足 */
  hint?: string;
}

const tls = (host: string, port: number): ServerEndpoint => ({ host, port, security: "tls" });
const startTls = (host: string, port: number): ServerEndpoint => ({
  host,
  port,
  security: "startTls",
});

export const CUSTOM_PROVIDER_ID = "custom";

export const PROVIDERS: Provider[] = [
  {
    id: "gmail",
    label: "Gmail",
    domains: ["gmail.com", "googlemail.com"],
    imap: tls("imap.gmail.com", 993),
    smtp: tls("smtp.gmail.com", 465),
    hint: "Google アカウントで 2 段階認証を有効にし、「アプリ パスワード」を発行して入力してください。",
  },
  {
    id: "icloud",
    label: "iCloud",
    domains: ["icloud.com", "me.com", "mac.com"],
    imap: tls("imap.mail.me.com", 993),
    smtp: startTls("smtp.mail.me.com", 587),
    hint: "Apple ID の「App 用パスワード」を発行して入力してください。",
  },
  {
    id: "yahoo-jp",
    label: "Yahoo! JAPAN",
    domains: ["yahoo.co.jp", "ymail.ne.jp"],
    imap: tls("imap.mail.yahoo.co.jp", 993),
    smtp: tls("smtp.mail.yahoo.co.jp", 465),
    hint: "Yahoo!メールの設定で「IMAP/POP/SMTP アクセス」を有効にしてください。",
  },
  {
    id: "fastmail",
    label: "Fastmail",
    domains: ["fastmail.com", "fastmail.fm"],
    imap: tls("imap.fastmail.com", 993),
    smtp: tls("smtp.fastmail.com", 465),
    hint: "Fastmail の設定で「アプリパスワード」を発行して入力してください。",
  },
  {
    id: CUSTOM_PROVIDER_ID,
    label: "その他 (手動設定)",
    domains: [],
    imap: tls("", 993),
    smtp: tls("", 465),
  },
];

export function providerForEmail(email: string): Provider | undefined {
  const domain = email.split("@")[1]?.trim().toLowerCase();
  return PROVIDERS.find((provider) => domain && provider.domains.includes(domain));
}
