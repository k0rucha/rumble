<script lang="ts">
  // 画面の骨組み: [フォルダ] [メール一覧] [本文] の 3 ペインと、その上に重なるダイアログ。
  import { onMount } from "svelte";
  import AccountDialog from "./components/AccountDialog.svelte";
  import ClassifierDialog from "./components/ClassifierDialog.svelte";
  import ComposeDialog from "./components/ComposeDialog.svelte";
  import SourceDialog from "./components/SourceDialog.svelte";
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
    const dialogOpen = store.draft ||
      store.accountDialogOpen ||
      store.classifierDialogOpen ||
      store.sourceDialogOpen;
    if (typing || dialogOpen || mod || event.altKey) return;

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

<main class:reading={store.selectedUid !== null} class:menu={store.sidebarOpen}>
  <Sidebar />
  <button class="scrim" aria-label="フォルダ一覧を閉じる" onclick={() => (store.sidebarOpen = false)}
  ></button>
  {#if store.location}
    <MessageList />
    <MessageView />
  {:else}
    <section class="empty muted">
      {#if store.accounts.length > 0}
        <button class="narrow-only" onclick={() => (store.sidebarOpen = true)}>フォルダを開く</button>
      {/if}
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
{#if store.classifierDialogOpen}
  <ClassifierDialog />
{/if}
{#if store.sourceDialogOpen}
  <SourceDialog />
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
    padding: 24px;
    text-align: center;
  }

  .scrim {
    display: none;
  }

  /* 狭い画面 (スマートフォン): 3 ペインを並べず、1 画面に 1 つだけ見せる。
     - 通常         : メール一覧
     - .reading     : 本文 (メールを開いている間)
     - .menu        : フォルダ一覧を左から引き出して重ねる */
  @media (max-width: 760px) {
    main {
      grid-template-columns: minmax(0, 1fr);
    }

    main > :global(*) {
      grid-column: 1;
      grid-row: 1;
    }

    main > :global(nav) {
      position: fixed;
      inset: 0 auto 0 0;
      z-index: 11;
      width: min(84vw, 320px);
      transform: translateX(-100%);
      transition: transform 0.18s ease-out;
    }

    main.menu > :global(nav) {
      transform: none;
    }

    @media (prefers-reduced-motion: reduce) {
      main > :global(nav) {
        transition: none;
      }
    }

    main.menu > .scrim {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 10;
      border: none;
      border-radius: 0;
      background: rgb(0 0 0 / 40%);
    }

    main.reading > :global(section),
    main:not(.reading) > :global(article) {
      display: none;
    }
  }
</style>
