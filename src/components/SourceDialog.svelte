<script lang="ts">
  // エンジニア向け: 開いているメールのヘッダー全文と、本文を含む生データを文字のまま見せる。
  // サーバーから受け取ったバイト列そのままで、デコードも整形もしない。
  import { fileSize } from "../lib/format";
  import { store } from "../lib/store.svelte";
  import type { MessageSource } from "../lib/types";
  import Dialog from "./Dialog.svelte";

  type Tab = "headers" | "source";

  let tab = $state<Tab>("headers");
  let source = $state<MessageSource | null>(null);
  let failed = $state(false);
  let copied = $state(false);

  $effect(() => {
    void store.loadSource().then((loaded) => {
      source = loaded ?? null;
      failed = loaded === undefined;
    });
  });

  const shown = $derived(tab === "headers" ? (source?.headers ?? "") : (source?.text ?? ""));

  /** ヘッダーを「名前」と「値 (折り返した続きの行を含む)」に分ける。名前を強調して読みやすくするため */
  const fields = $derived.by(() => {
    const result: { name: string; value: string }[] = [];
    for (const line of (source?.headers ?? "").split(/\r?\n/)) {
      const colon = line.indexOf(":");
      const last = result.at(-1);
      if (/^[ \t]/.test(line) && last) last.value += `\n${line}`;
      else if (colon > 0) result.push({ name: line.slice(0, colon), value: line.slice(colon + 1) });
      else if (line !== "" && last) last.value += `\n${line}`;
    }
    return result;
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(shown);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      store.notify("error", "コピーできませんでした。文字を選択してコピーしてください。");
    }
  }
</script>

<Dialog title="メールのソース" width={900} onclose={() => (store.sourceDialogOpen = false)}>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === "headers"} onclick={() => (tab = "headers")}>
      ヘッダー
    </button>
    <button role="tab" aria-selected={tab === "source"} onclick={() => (tab = "source")}>
      ソース全体
    </button>
  </div>

  {#if source}
    {#if tab === "headers"}
      <pre class="selectable">{#each fields as field}<span class="name">{field.name}:</span
          >{field.value}{"\n"}{/each}</pre>
    {:else}
      <pre class="selectable">{source.text}</pre>
      {#if source.truncated}
        <p class="muted">
          大きなメールのため、先頭の {fileSize(source.text.length)} だけを表示しています (全体は
          {fileSize(source.totalBytes)})。
        </p>
      {/if}
    {/if}
  {:else}
    <p class="muted">{failed ? "ソースを取得できませんでした" : "取得中…"}</p>
  {/if}

  {#snippet footer()}
    <button disabled={!source} onclick={copy}>{copied ? "コピーしました" : "コピー"}</button>
    {#if source}
      <span class="muted">{fileSize(source.totalBytes)}</span>
    {/if}
    <span class="spacer"></span>
    <button onclick={() => (store.sourceDialogOpen = false)}>閉じる</button>
  {/snippet}
</Dialog>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 10px;
  }

  .tabs button[aria-selected="true"] {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  pre {
    flex: 1;
    min-height: 240px;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-list);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .name {
    color: var(--accent);
    font-weight: 600;
  }

  p {
    margin: 8px 0 0;
  }

  .spacer {
    flex: 1;
  }
</style>
