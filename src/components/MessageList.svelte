<script lang="ts">
  // 中央ペイン: 開いているフォルダのメール一覧と検索欄。
  // 一覧はサーバーから少しずつ取得し、下までスクロールすると続きを読み込む。
  import { addressLabel, CATEGORY_LABELS, mailboxLabel, shortDate } from "../lib/format";
  import { store } from "../lib/store.svelte";
  import type { Category, MessageSummary } from "../lib/types";
  import Icon from "./Icon.svelte";

  let query = $state("");

  // フォルダを移ったり検索を解除したりしたら、検索欄も合わせる
  $effect(() => {
    query = store.activeSearch ?? "";
  });

  // 送信済み・下書きでは「誰から」より「誰宛て」が知りたい
  const showRecipient = $derived(
    store.location?.mailbox.role === "sent" || store.location?.mailbox.role === "drafts",
  );

  function counterpart(message: MessageSummary): string {
    if (!showRecipient) return addressLabel(message.from);
    return message.to.map((address) => addressLabel(address)).join(", ") || "(宛先なし)";
  }

  const categories = Object.entries(CATEGORY_LABELS) as [Exclude<Category, "other">, string][];

  function onScroll(event: Event) {
    const list = event.currentTarget as HTMLElement;
    if (list.scrollHeight - list.scrollTop - list.clientHeight < 300) void store.loadMore();
  }

  function onSearch(event: SubmitEvent) {
    event.preventDefault();
    void store.search(query);
  }
</script>

<section>
  <header>
    <div class="title">
      <button
        class="ghost narrow-only"
        aria-label="フォルダ一覧を開く"
        onclick={() => (store.sidebarOpen = true)}
      >
        <Icon name="menu" />
      </button>
      <h1>{store.location ? mailboxLabel(store.location.mailbox) : ""}</h1>
      <span class="muted count">
        {#if !store.isPlainListing}
          {store.total} 件一致
        {:else}
          {store.total} 通
        {/if}
      </span>
      <button
        class="ghost"
        title="サーバーの最新状態に更新"
        aria-label="更新"
        disabled={store.listLoading}
        onclick={() => store.refresh()}
      >
        <Icon name="refresh" />
      </button>
    </div>

    <form class="search" onsubmit={onSearch}>
      <Icon name="search" size={14} />
      <input
        type="search"
        placeholder="このフォルダをサーバーで検索"
        bind:value={query}
        onkeydown={(event) => event.key === "Escape" && store.clearSearch()}
      />
      {#if store.activeSearch !== null}
        <button type="button" class="ghost" aria-label="検索を解除" onclick={() => store.clearSearch()}>
          <Icon name="close" size={14} />
        </button>
      {/if}
    </form>

    {#if store.classifierEnabled}
      <div class="categories" role="group" aria-label="区分で絞り込む">
        <button
          class="chip"
          class:active={store.activeCategory === null}
          onclick={() => store.filterByCategory(null)}
        >
          すべて
        </button>
        {#each categories as [category, label] (category)}
          <button
            class="chip {category}"
            class:active={store.activeCategory === category}
            onclick={() => store.filterByCategory(category)}
          >
            {label}
          </button>
        {/each}
      </div>
    {/if}
  </header>

  <div class="list" onscroll={onScroll}>
    {#each store.messages as message (message.uid)}
      <button
        class="row"
        class:selected={message.uid === store.selectedUid}
        class:unread={!message.seen}
        onclick={() => store.select(message.uid)}
      >
        <span class="dot" aria-label={message.seen ? "" : "未読"}></span>
        <span class="who">{counterpart(message)}</span>
        <span class="date muted">{shortDate(message.date)}</span>
        <span class="subject">{message.subject || "(件名なし)"}</span>
        <span class="marks">
          {#if message.category && message.category !== "other"}
            <span class="tag {message.category}">{CATEGORY_LABELS[message.category]}</span>
          {/if}
          {#if message.answered}<Icon name="reply" size={13} />{/if}
          {#if message.hasAttachment}<Icon name="attachment" size={13} />{/if}
          {#if message.flagged}<span class="star"><Icon name="flagged" size={13} filled /></span>{/if}
        </span>
      </button>
    {/each}

    {#if store.listLoading}
      <p class="status muted">読み込み中…</p>
    {:else if store.messages.length === 0}
      <p class="status muted">
        {store.isPlainListing ? "メールはありません" : "一致するメールはありません"}
      </p>
    {:else if !store.isPlainListing && store.messages.length < store.total}
      <p class="status muted">新しい順に {store.messages.length} 件を表示しています</p>
    {/if}
  </div>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-list);
    border-right: 1px solid var(--border);
  }

  header {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 8px;
  }

  h1 {
    margin: 0;
    font-size: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    flex: 1;
    font-size: 11px;
    white-space: nowrap;
  }

  .title button {
    align-self: center;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px 0 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text-muted);
  }

  .search:focus-within {
    border-color: var(--accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    padding: 6px 0;
    background: transparent;
    color: var(--text);
  }

  .search input::-webkit-search-cancel-button {
    display: none;
  }

  .categories {
    display: flex;
    gap: 6px;
    margin-top: 8px;
    overflow-x: auto;
  }

  .chip {
    padding: 2px 10px;
    border-radius: 12px;
    color: var(--category, var(--text));
    font-size: 12px;
  }

  .chip.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  /* 区分ごとの色。一覧の印 (.tag) と絞り込みボタン (.chip) で共通 */
  .personal {
    --category: #2f9e44;
  }

  .notification {
    --category: #1c7ed6;
  }

  .receipt {
    --category: #e67700;
  }

  .newsletter {
    --category: #9c36b5;
  }


  .tag {
    padding: 0 6px;
    border: 1px solid var(--category);
    border-radius: 8px;
    color: var(--category);
    font-size: 10px;
    line-height: 15px;
  }

  .list {
    flex: 1;
    overflow-y: auto;
  }

  .row {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) auto;
    grid-template-areas:
      "dot who date"
      ". subject marks";
    column-gap: 6px;
    row-gap: 2px;
    align-items: center;
    width: 100%;
    padding: 9px 12px 9px 8px;
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
    text-align: left;
    white-space: nowrap;
  }

  .row.selected,
  .row.selected:hover {
    background: var(--bg-selected);
  }

  .dot {
    grid-area: dot;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .unread .dot {
    background: var(--accent);
  }

  .who {
    grid-area: who;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .unread .who,
  .unread .subject {
    font-weight: 700;
  }

  .date {
    grid-area: date;
    font-size: 11px;
  }

  .subject {
    grid-area: subject;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-muted);
  }

  .unread .subject {
    color: var(--text);
  }

  .marks {
    grid-area: marks;
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text-muted);
  }

  .star {
    display: flex;
    color: #f59f00;
  }

  .status {
    padding: 16px;
    text-align: center;
  }
</style>
