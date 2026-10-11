<script lang="ts">
  // 左ペイン: アカウントごとのフォルダ一覧。
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { mailboxLabel } from "../lib/format";
  import { store } from "../lib/store.svelte";
  import type { Account, Mailbox } from "../lib/types";
  import Icon from "./Icon.svelte";

  function isCurrent(accountId: string, mailbox: Mailbox): boolean {
    return (
      store.location?.accountId === accountId && store.location.mailbox.path === mailbox.path
    );
  }

  async function removeAccount(account: Account) {
    const ok = await confirm(
      `${account.email} を rumble から削除しますか?\nサーバー上のメールはそのまま残ります。`,
      { title: "アカウントの削除", kind: "warning" },
    );
    if (ok) await store.removeAccount(account.id);
  }
</script>

<nav>
  <button class="primary compose" onclick={() => store.compose()} disabled={!store.accounts.length}>
    <Icon name="compose" /> 新規メール
  </button>

  <div class="accounts">
    {#each store.accounts as account (account.id)}
      <section>
        <header>
          <span class="email" title={account.email}>{account.email}</span>
          <button
            class="ghost remove"
            title="このアカウントを削除"
            aria-label="このアカウントを削除"
            onclick={() => removeAccount(account)}
          >
            <Icon name="close" size={12} />
          </button>
        </header>

        {#each store.mailboxes[account.id] ?? [] as mailbox (mailbox.path)}
          <button
            class="mailbox"
            class:current={isCurrent(account.id, mailbox)}
            disabled={!mailbox.selectable}
            style:padding-left="{10 + (mailbox.role === 'other' ? mailbox.depth * 14 : 0)}px"
            onclick={() => store.openMailbox(account.id, mailbox)}
          >
            <Icon name={mailbox.role} />
            <span class="name">{mailboxLabel(mailbox)}</span>
            {#if mailbox.role === "inbox" && store.inboxUnseen[account.id]}
              <span class="badge">{store.inboxUnseen[account.id]}</span>
            {/if}
          </button>
        {/each}
      </section>
    {/each}
  </div>

  <button class="ghost add" onclick={() => (store.accountDialogOpen = true)}>
    <Icon name="plus" /> アカウントを追加
  </button>
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 12px 8px 8px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
  }

  .compose {
    justify-content: center;
    padding: 8px;
    margin: 0 4px 12px;
  }

  .accounts {
    flex: 1;
    overflow-y: auto;
  }

  section {
    margin-bottom: 14px;
  }

  header {
    display: flex;
    align-items: center;
    padding: 4px 4px 4px 10px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
  }

  .email {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remove {
    padding: 3px;
    opacity: 0;
  }

  header:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }

  .mailbox {
    width: 100%;
    border: none;
    background: transparent;
    padding: 6px 10px;
    gap: 8px;
    text-align: left;
  }

  .mailbox.current {
    background: var(--bg-selected);
    font-weight: 600;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    min-width: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 600;
    text-align: center;
  }

  .add {
    justify-content: flex-start;
    color: var(--text-muted);
  }
</style>
