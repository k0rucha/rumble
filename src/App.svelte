<script lang="ts">
  // 画面の骨組み: [フォルダ] [メール一覧] [本文] の 3 ペインと、その上に重なるダイアログ。
  import { onMount } from "svelte";
  import AccountDialog from "./components/AccountDialog.svelte";
  import ComposeDialog from "./components/ComposeDialog.svelte";
  import MessageList from "./components/MessageList.svelte";
  import MessageView from "./components/MessageView.svelte";
  import Notices from "./components/Notices.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import { store } from "./lib/store.svelte";

  onMount(() => void store.start());

  function onKeydown(event: KeyboardEvent) {
    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key === "n") {
      event.preventDefault();
      store.compose();
      return;
    }
    // 以降は一覧の操作。入力中やダイアログ表示中は反応しない
    const typing = (event.target as HTMLElement).closest("input, textarea, select") !== null;
    if (typing || store.draft || store.accountDialogOpen || mod || event.altKey) return;

    switch (event.key) {
      case "ArrowDown":
      case "j":
        event.preventDefault();
        void store.selectNeighbor(1);
        break;
      case "ArrowUp":
      case "k":
        event.preventDefault();
        void store.selectNeighbor(-1);
        break;
      case "Delete":
      case "Backspace":
        void store.deleteSelected();
        break;
      case "r":
        store.reply({ all: event.shiftKey });
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main>
  <Sidebar />
  {#if store.location}
    <MessageList />
    <MessageView />
  {:else}
    <section class="empty muted">
      {#if store.ready && store.accounts.length === 0}
        <p>メールアカウントを追加すると、ここにメールが表示されます。</p>
        <button class="primary" onclick={() => (store.accountDialogOpen = true)}>
          アカウントを追加
        </button>
      {/if}
    </section>
  {/if}
</main>

{#if store.draft}
  <ComposeDialog />
{/if}
{#if store.accountDialogOpen}
  <AccountDialog />
{/if}
<Notices />

<style>
  main {
    display: grid;
    grid-template-columns: 230px 360px minmax(0, 1fr);
    height: 100%;
  }

  .empty {
    grid-column: 2 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }
</style>
