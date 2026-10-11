// 画面全体の状態と、利用者の操作に対応するアクション。
//
// 状態はすべて「いまサーバーから見えているものの写し」で、どこにも保存しない。
// 操作は先に画面へ反映し (楽観的更新)、サーバーが拒否したら一覧を取り直して
// サーバーの状態に合わせ直す。

import { dismissTopmost } from "./back";
import * as backend from "./backend";
import { addressLabel, fullDate, quote, splitRecipients, withPrefix } from "./format";
import type {
  Account,
  Category,
  Mailbox,
  MessageBody,
  MessageSource,
  MessageSummary,
  NewAccount,
  OutgoingMessage,
  SenderAuth,
} from "./types";

/** 一覧を 1 回に取得する件数 */
const PAGE_SIZE = 50;
/** 自動分類を 1 回に依頼する件数 */
const CLASSIFY_BATCH_SIZE = 50;
/** 自動更新で取り直す件数の上限 */
const MAX_REFRESH_SIZE = 200;

/** 1 画面に 1 ペインだけを出す幅か (styles.css の --narrow と同じ値) */
const isNarrowScreen = () => window.matchMedia("(max-width: 760px)").matches;

export interface Location {
  accountId: string;
  mailbox: Mailbox;
}

/** 作成中のメール。メモリ上にしか無く、閉じれば消える */
export interface Draft {
  accountId: string;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  /** 添付するローカルファイルのパス */
  attachments: string[];
  inReplyTo: string | null;
  references: string[];
  repliedTo: { mailbox: string; uid: number } | null;
}

export interface Notice {
  id: number;
  kind: "error" | "info";
  text: string;
}

class MailStore {
  // ---- アカウントとフォルダ ----
  accounts = $state<Account[]>([]);
  mailboxes = $state<Record<string, Mailbox[]>>({});
  inboxUnseen = $state<Record<string, number>>({});
  /** 起動時の読み込みが済んだか */
  ready = $state(false);

  // ---- 開いているフォルダの一覧 ----
  location = $state<Location | null>(null);
  messages = $state<MessageSummary[]>([]);
  total = $state(0);
  listLoading = $state(false);
  /** 検索結果を表示中なら、その検索語 */
  activeSearch = $state<string | null>(null);
  /** 区分で絞り込み中なら、その区分 (検索とは同時に使わない) */
  activeCategory = $state<Category | null>(null);

  // ---- 自動分類 ----
  /** API キーが登録済みで、自動分類が有効か */
  classifierEnabled = $state(false);
  classifierDialogOpen = $state(false);

  // ---- 開いているメール ----
  selectedUid = $state<number | null>(null);
  openMessage = $state<MessageBody | null>(null);
  messageLoading = $state(false);
  /** 開いているメールの送信元の検証結果。検証中や対象外のフォルダでは null */
  senderAuth = $state<SenderAuth | null>(null);
  senderAuthLoading = $state(false);
  sourceDialogOpen = $state(false);

  // ---- ダイアログと通知 ----
  draft = $state<Draft | null>(null);
  sending = $state(false);
  accountDialogOpen = $state(false);
  /** 狭い画面でフォルダ一覧 (引き出し) を開いているか */
  sidebarOpen = $state(false);
  notices = $state<Notice[]>([]);

  // 古いリクエストの結果で新しい表示を上書きしないための通し番号
  #listRequest = 0;
  #messageRequest = 0;
  #noticeId = 0;
  /** 分類を依頼中の UID (二重に依頼しないため) */
  #classifying = new Set<number>();
  /** 分類に失敗したら、この起動中は自動では再試行しない (エラーを出し続けないため) */
  #classifierFailed = false;

  /** 検索や絞り込みをしていない、フォルダそのものの一覧か */
  get isPlainListing(): boolean {
    return this.activeSearch === null && this.activeCategory === null;
  }

  get hasMore(): boolean {
    return this.isPlainListing && this.messages.length < this.total;
  }

  get selectedSummary(): MessageSummary | undefined {
    return this.messages.find((message) => message.uid === this.selectedUid);
  }

  get currentMailboxes(): Mailbox[] {
    return this.location ? (this.mailboxes[this.location.accountId] ?? []) : [];
  }

  // ======================================================================
  // 起動
  // ======================================================================

  async start() {
    await backend.onMailEvent((event) => this.#onMailboxChanged(event.accountId, event.mailbox));
    await backend.onComposeRequest((address) =>
      this.compose({ to: decodeURIComponent(address) }),
    );
    await backend.onBackButton(() => this.back());

    this.classifierEnabled = await backend.classifierEnabled().catch(() => false);
    this.accounts = (await this.#attempt(() => backend.listAccounts())) ?? [];
    this.ready = true;
    if (this.accounts.length === 0) {
      this.accountDialogOpen = true;
      return;
    }
    await Promise.all(this.accounts.map((account) => this.#loadMailboxes(account.id)));
    await this.#openInbox(this.accounts[0].id);
  }

  /**
   * Android の戻るボタン。手前にあるものから順に 1 つずつ戻る:
   * ダイアログ → フォルダ一覧の引き出し → 開いているメール → 検索結果 → アプリを終了
   */
  back() {
    if (dismissTopmost()) return;
    if (this.sidebarOpen) this.sidebarOpen = false;
    else if (this.selectedUid !== null) this.closeMessage();
    else if (this.activeSearch !== null) void this.clearSearch();
    else if (this.activeCategory !== null) void this.filterByCategory(null);
    else void backend.exitApp();
  }

  // ======================================================================
  // アカウント
  // ======================================================================

  /** 失敗したら例外を投げる (ダイアログ側でその場に表示する) */
  async addAccount(input: NewAccount) {
    const account = await backend.addAccount(input);
    this.accounts.push(account);
    this.accountDialogOpen = false;
    await this.#loadMailboxes(account.id);
    await this.#openInbox(account.id);
  }

  async removeAccount(accountId: string) {
    if (!(await this.#succeeds(() => backend.removeAccount(accountId)))) return;

    this.accounts = this.accounts.filter((account) => account.id !== accountId);
    delete this.mailboxes[accountId];
    delete this.inboxUnseen[accountId];
    if (this.location?.accountId === accountId) {
      this.#clearList();
      this.location = null;
      if (this.accounts.length > 0) await this.#openInbox(this.accounts[0].id);
    }
  }

  // ======================================================================
  // フォルダと一覧
  // ======================================================================

  async openMailbox(accountId: string, mailbox: Mailbox) {
    if (!mailbox.selectable) return;
    this.sidebarOpen = false;
    this.location = { accountId, mailbox };
    this.#clearList();
    await this.#fetchList({ offset: 0, limit: PAGE_SIZE, replace: true });
  }

  /** 表示中の一覧をサーバーの最新状態に合わせる */
  async refresh({ quiet = false } = {}) {
    if (!this.location) return;
    if (this.activeSearch !== null) return this.search(this.activeSearch);
    if (this.activeCategory !== null) return this.filterByCategory(this.activeCategory);

    const limit = Math.min(Math.max(this.messages.length, PAGE_SIZE), MAX_REFRESH_SIZE);
    await this.#fetchList({ offset: 0, limit, replace: true, quiet });
    void this.#refreshUnseen(this.location.accountId);
  }

  async loadMore() {
    if (!this.hasMore || this.listLoading) return;
    await this.#fetchList({ offset: this.messages.length, limit: PAGE_SIZE, replace: false });
  }

  /** サーバー側で検索する。空文字なら通常の一覧に戻る */
  async search(query: string) {
    const location = this.location;
    if (!location) return;
    if (query.trim() === "") return this.clearSearch();

    this.activeSearch = query.trim();
    this.activeCategory = null;
    await this.#showFound(() =>
      backend.searchMessages(location.accountId, location.mailbox.path, query),
    );
  }

  async clearSearch() {
    if (this.activeSearch === null) return;
    this.activeSearch = null;
    await this.#fetchList({ offset: 0, limit: PAGE_SIZE, replace: true });
  }

  /** 自動分類の区分で絞り込む (サーバー側のキーワード検索)。null なら解除 */
  async filterByCategory(category: Category | null) {
    const location = this.location;
    if (!location) return;
    this.activeSearch = null;
    this.activeCategory = category;
    if (category === null) {
      await this.#fetchList({ offset: 0, limit: PAGE_SIZE, replace: true });
      return;
    }
    await this.#showFound(() =>
      backend.listByCategory(location.accountId, location.mailbox.path, category),
    );
  }

  // ======================================================================
  // 自動分類
  // ======================================================================

  /** 失敗したら例外を投げる (ダイアログ側でその場に表示する) */
  async enableClassifier(apiKey: string) {
    await backend.enableClassifier(apiKey);
    this.classifierEnabled = true;
    this.#classifierFailed = false;
    this.classifierDialogOpen = false;
    void this.#classifyUnsorted();
  }

  async disableClassifier() {
    if (!(await this.#succeeds(() => backend.disableClassifier()))) return;
    this.classifierEnabled = false;
    this.classifierDialogOpen = false;
    if (this.activeCategory !== null) await this.filterByCategory(null);
  }

  // ======================================================================
  // メールを開く・操作する
  // ======================================================================

  async select(uid: number) {
    const location = this.location;
    if (!location || uid === this.selectedUid) return;

    const request = ++this.#messageRequest;
    this.selectedUid = uid;
    this.openMessage = null;
    this.messageLoading = true;
    this.senderAuth = null;
    this.senderAuthLoading = false;
    const body = await this.#attempt(() =>
      backend.getMessage(location.accountId, location.mailbox.path, uid),
    );
    if (request !== this.#messageRequest) return;
    this.messageLoading = false;
    this.openMessage = body ?? null;
    if (body) void this.#verifySender(location, uid, request);

    // 開けたら既読にする
    if (body && this.selectedSummary && !this.selectedSummary.seen) {
      await this.setSeen(uid, true);
    }
  }

  /** 開いているメールを閉じて一覧に戻る (狭い画面用) */
  closeMessage() {
    this.#forgetOpenMessage();
  }

  /** 開いているメールのヘッダー全文と生データを取得する (ソース表示用) */
  async loadSource(): Promise<MessageSource | undefined> {
    const location = this.location;
    const uid = this.selectedUid;
    if (!location || uid === null) return undefined;
    return this.#attempt(() => backend.messageSource(location.accountId, location.mailbox.path, uid));
  }

  /**
   * 送信元を検証する。本文は先に表示しておき、結果は後から届く。
   * 自分が出したメール (送信済み・下書き) は検証の対象にしない。
   */
  async #verifySender(location: Location, uid: number, request: number) {
    if (location.mailbox.role === "sent" || location.mailbox.role === "drafts") return;
    this.senderAuthLoading = true;
    // 検証できなくても本文は読めるので、失敗は通知せず「結果なし」にとどめる
    const auth = await backend
      .verifySender(location.accountId, location.mailbox.path, uid)
      .catch(() => null);
    if (request !== this.#messageRequest) return;
    this.senderAuthLoading = false;
    this.senderAuth = auth;
  }

  #forgetOpenMessage() {
    this.#messageRequest++;
    this.selectedUid = null;
    this.openMessage = null;
    this.messageLoading = false;
    this.senderAuth = null;
    this.senderAuthLoading = false;
    this.sourceDialogOpen = false;
  }

  /** 一覧の前後のメールへ移動する (キーボード操作用) */
  async selectNeighbor(step: 1 | -1) {
    const index = this.messages.findIndex((message) => message.uid === this.selectedUid);
    const next = this.messages[index === -1 ? 0 : index + step];
    if (next) await this.select(next.uid);
  }

  async setSeen(uid: number, seen: boolean) {
    const message = this.messages.find((candidate) => candidate.uid === uid);
    if (!message || message.seen === seen) return;
    message.seen = seen;
    this.#adjustUnseen(seen ? -1 : 1);
    await this.#syncFlag(uid, "seen", seen);
  }

  async setFlagged(uid: number, flagged: boolean) {
    const message = this.messages.find((candidate) => candidate.uid === uid);
    if (!message || message.flagged === flagged) return;
    message.flagged = flagged;
    await this.#syncFlag(uid, "flagged", flagged);
  }

  async moveSelected(destination: Mailbox) {
    const location = this.location;
    const uid = this.selectedUid;
    if (!location || uid === null || destination.path === location.mailbox.path) return;
    await this.#removeFromList(uid, () =>
      backend.moveMessages(location.accountId, location.mailbox.path, [uid], destination.path),
    );
  }

  /** ゴミ箱へ移動する。ゴミ箱の中では完全に削除される (バックエンドが判断する) */
  async deleteSelected() {
    const location = this.location;
    const uid = this.selectedUid;
    if (!location || uid === null) return;
    await this.#removeFromList(uid, () =>
      backend.deleteMessages(location.accountId, location.mailbox.path, [uid]),
    );
  }

  async saveAttachment(attachmentIndex: number, destination: string) {
    const location = this.location;
    const uid = this.selectedUid;
    if (!location || uid === null) return;
    const saved = await this.#succeeds(() =>
      backend.saveAttachment(
        location.accountId,
        location.mailbox.path,
        uid,
        attachmentIndex,
        destination,
      ),
    );
    if (saved) this.notify("info", "添付ファイルを保存しました");
  }

  // ======================================================================
  // 作成と送信
  // ======================================================================

  compose(initial: Partial<Draft> = {}) {
    const accountId = this.location?.accountId ?? this.accounts[0]?.id;
    if (!accountId) return;
    this.draft = {
      accountId,
      to: "",
      cc: "",
      bcc: "",
      subject: "",
      body: "",
      attachments: [],
      inReplyTo: null,
      references: [],
      repliedTo: null,
      ...initial,
    };
  }

  reply({ all }: { all: boolean }) {
    const message = this.openMessage;
    const location = this.location;
    if (!message || !location) return;

    const myEmail = this.accounts
      .find((account) => account.id === location.accountId)
      ?.email.toLowerCase();
    const recipients = message.replyTo.length > 0 ? message.replyTo : message.from ? [message.from] : [];
    const to = recipients.map((address) => address.email);
    // 全員に返信: 元の To / Cc から、自分と To に入れた人を除いて Cc へ
    const cc = all
      ? [...message.to, ...message.cc]
          .map((address) => address.email)
          .filter((email, index, list) => list.indexOf(email) === index)
          .filter((email) => email.toLowerCase() !== myEmail && !to.includes(email))
      : [];

    this.compose({
      to: to.join(", "),
      cc: cc.join(", "),
      subject: withPrefix(message.subject, "Re"),
      body: `\n\n${fullDate(message.date)} ${addressLabel(message.from)}:\n${quote(message.text)}\n`,
      inReplyTo: message.messageId,
      references: [...message.references, ...(message.messageId ? [message.messageId] : [])],
      repliedTo: { mailbox: location.mailbox.path, uid: message.uid },
    });
  }

  /** 本文をテキストとして転送する (元の添付ファイルは引き継がない) */
  forward() {
    const message = this.openMessage;
    if (!message) return;
    const header = [
      "---------- 転送メッセージ ----------",
      `差出人: ${message.from ? `${addressLabel(message.from)} <${message.from.email}>` : ""}`,
      `日時: ${fullDate(message.date)}`,
      `件名: ${message.subject}`,
      `宛先: ${message.to.map((address) => address.email).join(", ")}`,
    ].join("\n");
    this.compose({
      subject: withPrefix(message.subject, "Fwd"),
      body: `\n\n${header}\n\n${message.text ?? ""}\n`,
    });
  }

  discardDraft() {
    this.draft = null;
  }

  async send() {
    const draft = this.draft;
    if (!draft || this.sending) return;

    const outgoing: OutgoingMessage = {
      to: splitRecipients(draft.to),
      cc: splitRecipients(draft.cc),
      bcc: splitRecipients(draft.bcc),
      subject: draft.subject,
      body: draft.body,
      attachments: draft.attachments,
      inReplyTo: draft.inReplyTo,
      references: draft.references,
      repliedTo: draft.repliedTo,
    };
    this.sending = true;
    const report = await this.#attempt(() => backend.sendMessage(draft.accountId, outgoing));
    this.sending = false;
    if (!report) return; // 失敗時は作成画面を残して書き直せるようにする

    this.draft = null;
    if (report.sentCopy.kind === "failed") {
      this.notify(
        "error",
        `送信しましたが、送信済みフォルダに控えを保存できませんでした: ${report.sentCopy.detail}`,
      );
    } else {
      this.notify("info", "送信しました");
    }
    if (draft.repliedTo) {
      const original = this.messages.find((message) => message.uid === draft.repliedTo?.uid);
      if (original) original.answered = true;
    }
  }

  // ======================================================================
  // 通知
  // ======================================================================

  notify(kind: Notice["kind"], text: string) {
    const id = ++this.#noticeId;
    this.notices.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), kind === "error" ? 10_000 : 4_000);
  }

  dismiss(id: number) {
    this.notices = this.notices.filter((notice) => notice.id !== id);
  }

  // ======================================================================
  // 内部
  // ======================================================================

  /** 値を返す処理用。失敗は通知に変え、undefined を返す */
  async #attempt<T extends object>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      return await action();
    } catch (error) {
      this.notify("error", String(error));
      return undefined;
    }
  }

  /** 値を返さない処理用。失敗は通知に変え、成否を返す */
  async #succeeds(action: () => Promise<void>): Promise<boolean> {
    try {
      await action();
      return true;
    } catch (error) {
      this.notify("error", String(error));
      return false;
    }
  }

  async #loadMailboxes(accountId: string) {
    const mailboxes = await this.#attempt(() => backend.listMailboxes(accountId));
    if (mailboxes) this.mailboxes[accountId] = mailboxes;
    void this.#refreshUnseen(accountId);
  }

  async #openInbox(accountId: string) {
    const inbox = this.mailboxes[accountId]?.find((mailbox) => mailbox.role === "inbox");
    if (inbox) await this.openMailbox(accountId, inbox);
  }

  async #refreshUnseen(accountId: string) {
    const inbox = this.mailboxes[accountId]?.find((mailbox) => mailbox.role === "inbox");
    if (!inbox) return;
    try {
      this.inboxUnseen[accountId] = (await backend.mailboxStatus(accountId, inbox.path)).unseen;
    } catch {
      // 未読数は補助的な表示なので、取れなくても騒がない
    }
  }

  #adjustUnseen(delta: number) {
    const location = this.location;
    if (location?.mailbox.role !== "inbox") return;
    const current = this.inboxUnseen[location.accountId] ?? 0;
    this.inboxUnseen[location.accountId] = Math.max(0, current + delta);
  }

  #clearList() {
    this.#listRequest++;
    this.messages = [];
    this.total = 0;
    this.activeSearch = null;
    this.activeCategory = null;
    this.#forgetOpenMessage();
  }

  /** 検索・絞り込みの結果を一覧に表示する */
  async #showFound(find: () => Promise<{ total: number; messages: MessageSummary[] }>) {
    const request = ++this.#listRequest;
    this.listLoading = true;
    const page = await this.#attempt(find);
    if (request !== this.#listRequest) return;
    this.listLoading = false;
    this.messages = page?.messages ?? [];
    this.total = page?.total ?? 0;
    void this.#classifyUnsorted();
  }

  /**
   * 表示中の一覧のうち、まだ区分の無いメールを分類する。
   * 結果はバックエンドがサーバー上のメールに付けるので、次回からは一覧と一緒に届く。
   */
  async #classifyUnsorted() {
    const location = this.location;
    if (!location || !this.classifierEnabled || this.#classifierFailed) return;

    const uids = this.messages
      .filter((message) => message.category === null && !this.#classifying.has(message.uid))
      .slice(0, CLASSIFY_BATCH_SIZE)
      .map((message) => message.uid);
    if (uids.length === 0) return;

    uids.forEach((uid) => this.#classifying.add(uid));
    const classified = await this.#attempt(() =>
      backend.classifyMessages(location.accountId, location.mailbox.path, uids),
    );
    uids.forEach((uid) => this.#classifying.delete(uid));
    if (!classified) {
      this.#classifierFailed = true;
      return;
    }
    // 待っている間に別のフォルダへ移っていたら、結果は捨てる (UID はフォルダごとの番号)
    if (this.location !== location) return;

    const categories = new Map(classified);
    for (const message of this.messages) {
      message.category ??= categories.get(message.uid) ?? null;
    }
    // 1 回ぶんに収まらなかった残りを続けて処理する。1 通も進まなければ打ち切る
    if (classified.length > 0) void this.#classifyUnsorted();
  }

  async #fetchList(options: { offset: number; limit: number; replace: boolean; quiet?: boolean }) {
    const location = this.location;
    if (!location) return;

    const request = ++this.#listRequest;
    if (!options.quiet) this.listLoading = true;
    const page = await this.#attempt(() =>
      backend.listMessages(
        location.accountId,
        location.mailbox.path,
        options.offset,
        options.limit,
      ),
    );
    if (request !== this.#listRequest) return;
    this.listLoading = false;
    if (!page) return;

    this.total = page.total;
    if (options.replace) {
      this.messages = page.messages;
    } else {
      // 読み込み中に新着があると範囲がずれて重複しうるので、UID で除く
      const known = new Set(this.messages.map((message) => message.uid));
      this.messages.push(...page.messages.filter((message) => !known.has(message.uid)));
    }
    void this.#classifyUnsorted();
  }

  async #syncFlag(uid: number, flag: "seen" | "flagged", enabled: boolean) {
    const location = this.location;
    if (!location) return;
    const synced = await this.#succeeds(() =>
      backend.setFlag(location.accountId, location.mailbox.path, [uid], flag, enabled),
    );
    if (!synced) await this.refresh({ quiet: true });
  }

  /** 一覧から 1 通を取り除き、次のメールを選ぶ。サーバー側の処理が失敗したら取り直す */
  async #removeFromList(uid: number, serverAction: () => Promise<void>) {
    const index = this.messages.findIndex((message) => message.uid === uid);
    if (index === -1) return;
    const [removed] = this.messages.splice(index, 1);
    this.total = Math.max(0, this.total - 1);
    if (!removed.seen) this.#adjustUnseen(-1);

    const next = this.messages[index] ?? this.messages[index - 1];
    this.closeMessage();
    // 3 ペイン表示では続けて次のメールを開く。1 画面ずつの表示では一覧に戻るだけ
    if (next && !isNarrowScreen()) void this.select(next.uid);

    if (!(await this.#succeeds(serverAction))) await this.refresh({ quiet: true });
  }

  async #onMailboxChanged(accountId: string, mailboxPath: string) {
    void this.#refreshUnseen(accountId);
    const location = this.location;
    const isShowing =
      location?.accountId === accountId &&
      location.mailbox.path.toUpperCase() === mailboxPath.toUpperCase();
    if (isShowing && this.isPlainListing) await this.refresh({ quiet: true });
  }
}

export const store = new MailStore();
