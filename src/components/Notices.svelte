<script lang="ts">
  // 画面右下に重ねて出す通知。エラーは長めに、完了報告は短めに表示される。
  import { store } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="notices">
  {#each store.notices as notice (notice.id)}
    <div class="notice selectable" class:error={notice.kind === "error"} role="status">
      <span>{notice.text}</span>
      <button class="ghost" aria-label="閉じる" onclick={() => store.dismiss(notice.id)}>
        <Icon name="close" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .notices {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 420px;
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 8px 10px 14px;
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius);
    background: var(--bg);
    box-shadow: var(--shadow);
  }

  .notice.error {
    border-left-color: var(--danger);
  }

  .notice span {
    flex: 1;
    overflow-wrap: anywhere;
  }
</style>
