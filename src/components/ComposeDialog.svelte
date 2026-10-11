<script lang="ts">
  // メール作成画面。新規・返信・転送で共通。
  // 書きかけの内容はメモリ上にしか無く、閉じると消える (下書き保存は未対応)。
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { store } from "../lib/store.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  // このコンポーネントは store.draft があるときだけ表示される (App.svelte)
  const draft = $derived(store.draft!);
  let showCcBcc = $state(store.draft!.cc !== "" || store.draft!.bcc !== "");

  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  async function attach() {
    const picked = await open({ multiple: true, title: "添付するファイルを選択" });
    if (picked) draft.attachments.push(...picked.filter((path) => !draft.attachments.includes(path)));
  }

  async function close() {
    const hasContent = draft.body.trim() !== "" || draft.to.trim() !== "";
    if (hasContent) {
      const ok = await confirm("書きかけのメールを破棄しますか?", { title: "メールの破棄", kind: "warning" });
      if (!ok) return;
    }
    store.discardDraft();
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") void store.send();
  }
</script>

<Dialog title={draft.inReplyTo ? "返信" : "新規メール"} width={720} onclose={close}>
  <!-- svelte-ignore a11y_no_static_element_interactions : Cmd+Enter での送信を拾うだけ -->
  <div class="form" onkeydown={onKeydown}>
    {#if store.accounts.length > 1}
      <label>
        <span>差出人</span>
        <select bind:value={draft.accountId} disabled={draft.repliedTo !== null}>
          {#each store.accounts as account (account.id)}
            <option value={account.id}>{account.email}</option>
          {/each}
        </select>
      </label>
    {/if}
    <label>
      <span>宛先</span>
      <!-- svelte-ignore a11y_autofocus : 開いたらすぐ入力を始められるように -->
      <input bind:value={draft.to} placeholder="カンマ区切りで複数指定" autofocus={!draft.to} />
      {#if !showCcBcc}
        <button type="button" class="ghost" onclick={() => (showCcBcc = true)}>Cc / Bcc</button>
      {/if}
    </label>
    {#if showCcBcc}
      <label><span>Cc</span><input bind:value={draft.cc} /></label>
      <label><span>Bcc</span><input bind:value={draft.bcc} /></label>
    {/if}
    <label><span>件名</span><input bind:value={draft.subject} /></label>

    <!-- svelte-ignore a11y_autofocus -->
    <textarea bind:value={draft.body} aria-label="本文" autofocus={!!draft.to}></textarea>

    {#if draft.attachments.length > 0}
      <ul>
        {#each draft.attachments as path, index (path)}
          <li title={path}>
            <Icon name="attachment" size={13} />
            {fileName(path)}
            <button
              type="button"
              class="ghost"
              aria-label="添付を外す"
              onclick={() => draft.attachments.splice(index, 1)}
            >
              <Icon name="close" size={12} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#snippet footer()}
    <button class="primary" disabled={store.sending} onclick={() => store.send()}>
      <Icon name="sent" />
      {store.sending ? "送信中…" : "送信"}
    </button>
    <button onclick={attach}><Icon name="attachment" /> 添付</button>
    <span class="hint muted">⌘/Ctrl + Enter で送信</span>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-height: 0;
  }

  label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  label span {
    width: 44px;
    color: var(--text-muted);
  }

  label input,
  label select {
    flex: 1;
    min-width: 0;
  }

  textarea {
    flex: 1;
    min-height: 280px;
    padding: 10px;
    resize: none;
    font-size: 14px;
    line-height: 1.6;
  }

  ul {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 2px 2px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  li button {
    padding: 3px;
  }

  .hint {
    margin-left: auto;
    font-size: 11px;
  }
</style>
