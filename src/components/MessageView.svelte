<script lang="ts">
  // 右ペイン: 選択したメールの本文と、そのメールに対する操作。
  import { confirm, save } from "@tauri-apps/plugin-dialog";
  import { addressFull, fileSize, fullDate, mailboxLabel } from "../lib/format";
  import { store } from "../lib/store.svelte";
  import type { Address, AttachmentInfo } from "../lib/types";
  import HtmlBody from "./HtmlBody.svelte";
  import Icon from "./Icon.svelte";

  const message = $derived(store.openMessage);
  const summary = $derived(store.selectedSummary);
  const inTrash = $derived(store.location?.mailbox.role === "trash");
  // Gmail には「アーカイブ」フォルダが無く、「すべてのメール」へ移すのがアーカイブにあたる
  const archive = $derived(
    store.currentMailboxes.find((mailbox) => mailbox.role === "archive") ??
      store.currentMailboxes.find((mailbox) => mailbox.role === "all"),
  );
  const moveTargets = $derived(
    store.currentMailboxes.filter(
      (mailbox) => mailbox.selectable && mailbox.path !== store.location?.mailbox.path,
    ),
  );

  // 外部画像を読み込むかどうかはメールごとに毎回たずねる (既定は読み込まない)
  let allowRemoteImages = $state(false);
  $effect(() => {
    void store.selectedUid;
    allowRemoteImages = false;
  });

  const addresses = (list: Address[]) => list.map(addressFull).join(", ");

  /** 本文中の URL をリンクにするため、テキストを「URL」と「それ以外」に分ける */
  function linkify(text: string): { text: string; isUrl: boolean }[] {
    return text
      .split(/(https?:\/\/[^\s<>"'）」]+)/g)
      .filter(Boolean)
      .map((part) => ({ text: part, isUrl: /^https?:\/\//.test(part) }));
  }

  async function deleteMessage() {
    if (inTrash) {
      const ok = await confirm("このメールを完全に削除しますか? 元に戻せません。", {
        title: "完全に削除",
        kind: "warning",
      });
      if (!ok) return;
    }
    await store.deleteSelected();
  }

  async function saveAttachment(attachment: AttachmentInfo) {
    const destination = await save({ defaultPath: attachment.filename });
    if (destination) await store.saveAttachment(attachment.index, destination);
  }

  function moveTo(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const destination = moveTargets.find((mailbox) => mailbox.path === select.value);
    select.value = "";
    if (destination) void store.moveSelected(destination);
  }
</script>

<article>
  {#if store.selectedUid === null}
    <p class="placeholder muted">メールを選択してください</p>
  {:else}
    <div class="toolbar">
      <button disabled={!message} onclick={() => store.reply({ all: false })}>
        <Icon name="reply" /> 返信
      </button>
      <button disabled={!message} onclick={() => store.reply({ all: true })}>
        <Icon name="replyAll" /> 全員に返信
      </button>
      <button disabled={!message} onclick={() => store.forward()}>
        <Icon name="forward" /> 転送
      </button>

      <span class="spacer"></span>

      {#if summary}
        <button
          class="ghost"
          title={summary.flagged ? "スターを外す" : "スターを付ける"}
          aria-label="スター"
          onclick={() => store.setFlagged(summary.uid, !summary.flagged)}
        >
          <span class:starred={summary.flagged}>
            <Icon name="flagged" filled={summary.flagged} />
          </span>
        </button>
        <button
          class="ghost"
          title="未読に戻す"
          aria-label="未読に戻す"
          disabled={!summary.seen}
          onclick={() => store.setSeen(summary.uid, false)}
        >
          <Icon name="unread" />
        </button>
      {/if}
      {#if archive && archive.path !== store.location?.mailbox.path}
        <button class="ghost" title="アーカイブ" aria-label="アーカイブ" onclick={() => store.moveSelected(archive)}>
          <Icon name="archive" />
        </button>
      {/if}
      <button
        class="ghost"
        title={inTrash ? "完全に削除" : "ゴミ箱へ移動"}
        aria-label="削除"
        onclick={deleteMessage}
      >
        <Icon name="trash" />
      </button>
      <select aria-label="フォルダへ移動" onchange={moveTo}>
        <option value="">移動…</option>
        {#each moveTargets as mailbox (mailbox.path)}
          <option value={mailbox.path}>{mailboxLabel(mailbox)}</option>
        {/each}
      </select>
    </div>

    {#if message}
      <header class="selectable">
        <h2>{message.subject || "(件名なし)"}</h2>
        <dl>
          <dt>差出人</dt>
          <dd>{message.from ? addressFull(message.from) : "(差出人なし)"}</dd>
          <dt>宛先</dt>
          <dd>{addresses(message.to)}</dd>
          {#if message.cc.length > 0}
            <dt>Cc</dt>
            <dd>{addresses(message.cc)}</dd>
          {/if}
          <dt>日時</dt>
          <dd>{fullDate(message.date)}</dd>
        </dl>
      </header>

      {#if message.attachments.length > 0}
        <ul class="attachments">
          {#each message.attachments as attachment (attachment.index)}
            <li>
              <button title="保存先を選んで保存" onclick={() => saveAttachment(attachment)}>
                <Icon name="attachment" size={14} />
                <span class="filename">{attachment.filename}</span>
                <span class="muted">{fileSize(attachment.size)}</span>
                <Icon name="download" size={14} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if message.html !== null}
        {#if message.hasRemoteContent && !allowRemoteImages}
          <div class="remote">
            <span>プライバシー保護のため、外部の画像を読み込んでいません。</span>
            <button onclick={() => (allowRemoteImages = true)}>画像を読み込む</button>
          </div>
        {/if}
        <HtmlBody html={message.html} {allowRemoteImages} />
      {:else}
        <pre class="text selectable">{#each linkify(message.text ?? "") as part}{#if part.isUrl}<a
                href={part.text}>{part.text}</a
              >{:else}{part.text}{/if}{/each}</pre>
      {/if}
    {:else if store.messageLoading}
      <p class="placeholder muted">サーバーから取得中…</p>
    {:else}
      <p class="placeholder muted">メールを表示できませんでした</p>
    {/if}
  {/if}
</article>

<style>
  article {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg);
  }

  .placeholder {
    margin: auto;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }

  .spacer {
    flex: 1;
  }

  .starred {
    display: flex;
    color: #f59f00;
  }

  select {
    max-width: 140px;
    padding: 5px 6px;
  }

  header {
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 18px;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }

  dl {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 2px 12px;
    margin: 0;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 10px 20px;
    border-bottom: 1px solid var(--border);
    list-style: none;
  }

  .filename {
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .remote {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 20px;
    background: var(--accent-soft);
    border-bottom: 1px solid var(--border);
  }

  .text {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 16px 20px;
    overflow: auto;
    font: inherit;
    font-size: 14px;
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
