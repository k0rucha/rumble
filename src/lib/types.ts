// バックエンド (crates/rumble-core/src/domain) の型と 1 対 1 で対応する。
// 片方を変えたらもう片方も必ず合わせること。

export type Security = "tls" | "startTls";

export interface ServerEndpoint {
  host: string;
  port: number;
  security: Security;
}

export interface Account {
  id: string;
  displayName: string;
  email: string;
  username: string;
  imap: ServerEndpoint;
  smtp: ServerEndpoint;
}

export interface NewAccount extends Omit<Account, "id"> {
  password: string;
}

export type MailboxRole =
  | "inbox"
  | "flagged"
  | "drafts"
  | "sent"
  | "archive"
  | "all"
  | "junk"
  | "trash"
  | "other";

export interface Mailbox {
  /** サーバー上の名前。バックエンドへはこれを渡す */
  path: string;
  /** 表示用の名前 */
  name: string;
  depth: number;
  role: MailboxRole;
  selectable: boolean;
}

export interface MailboxStatus {
  total: number;
  unseen: number;
}

export interface Address {
  name: string | null;
  email: string;
}

/** 自動分類の区分 (crates/rumble-core/src/domain/category.rs) */
export type Category = "personal" | "notification" | "receipt" | "newsletter" | "other";

export interface MessageSummary {
  uid: number;
  subject: string;
  from: Address | null;
  to: Address[];
  /** UNIX 秒 */
  date: number | null;
  size: number;
  seen: boolean;
  flagged: boolean;
  answered: boolean;
  hasAttachment: boolean;
  /** 自動分類の結果。まだ分類していなければ null */
  category: Category | null;
}

export interface MessagePage {
  total: number;
  messages: MessageSummary[];
}

export interface AttachmentInfo {
  index: number;
  filename: string;
  mimeType: string;
  size: number;
}

export interface MessageBody {
  uid: number;
  subject: string;
  from: Address | null;
  to: Address[];
  cc: Address[];
  replyTo: Address[];
  date: number | null;
  messageId: string | null;
  references: string[];
  /** サニタイズ済み HTML */
  html: string | null;
  text: string | null;
  hasRemoteContent: boolean;
  attachments: AttachmentInfo[];
}

/** 送信元の総合判定 (crates/rumble-core/src/domain/authenticity.rs) */
export type TrustLevel = "verified" | "reported" | "partial" | "failed" | "unknown";

export type CheckStatus = "pass" | "fail" | "inconclusive" | "none";

/** rumble が自分で検算した DKIM 署名 */
export interface SignatureCheck {
  domain: string;
  selector: string;
  status: CheckStatus;
  /** 差出人欄のドメインの署名とみなせるか */
  aligned: boolean;
  detail: string | null;
}

export type DmarcPolicy = "none" | "quarantine" | "reject";

/** 受信サーバーが Authentication-Results に書いた検査結果 */
export interface ReportedCheck {
  method: string;
  status: CheckStatus;
  result: string;
  domain: string | null;
}

export interface ReportedAuth {
  /** 検査したと名乗っているサーバー */
  verifier: string;
  checks: ReportedCheck[];
}

export interface BrandLogo {
  domain: string;
  dataUrl: string;
  /** 証明書 (VMC) が添えられているか。rumble は検証していない */
  hasCertificate: boolean;
}

export interface SenderAuth {
  level: TrustLevel;
  fromDomain: string | null;
  signatures: SignatureCheck[];
  dmarcPolicy: DmarcPolicy | null;
  reported: ReportedAuth | null;
  logo: BrandLogo | null;
  /** DNS に問い合わせられず、自前の検算ができなかった */
  lookupFailed: boolean;
}

/** メールの生データ (エンジニア向けの表示用) */
export interface MessageSource {
  headers: string;
  text: string;
  totalBytes: number;
  truncated: boolean;
}

export type MessageFlag = "seen" | "flagged" | "answered";

export interface MessageRef {
  mailbox: string;
  uid: number;
}

export interface OutgoingMessage {
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body: string;
  attachments: string[];
  inReplyTo: string | null;
  references: string[];
  repliedTo: MessageRef | null;
}

export type SentCopy =
  | { kind: "serverManaged" }
  | { kind: "appended" }
  | { kind: "failed"; detail: string };

export interface SendReport {
  sentCopy: SentCopy;
}

export type MailEvent = { type: "mailboxChanged"; accountId: string; mailbox: string };
