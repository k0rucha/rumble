<script lang="ts">
  // 画面中央に重ねるダイアログの枠。中身は呼び出し側が渡す。
  import type { Snippet } from "svelte";
  import { dismissOnBack } from "../lib/back";
  import Icon from "./Icon.svelte";

  let {
    title,
    width = 520,
    onclose,
    children,
    footer,
  }: {
    title: string;
    width?: number;
    onclose: () => void;
    children: Snippet;
    footer: Snippet;
  } = $props();

  // 表示されている間は、Android の戻るボタンでも閉じられるようにする (Esc と同じ扱い)
  $effect(() => dismissOnBack(() => onclose()));
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onclose()} />

<div class="backdrop">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title} style:width="{width}px">
    <header>
      <h2>{title}</h2>
      <button class="ghost" aria-label="閉じる" onclick={onclose}><Icon name="close" /></button>
    </header>
    <div class="body">{@render children()}</div>
    <footer>{@render footer()}</footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgb(0 0 0 / 35%);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    max-width: 100%;
    max-height: 100%;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg);
    box-shadow: var(--shadow);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 12px 12px 18px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    font-size: 14px;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 18px;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
  }

  /* 狭い画面では全画面で表示する */
  @media (max-width: 760px) {
    .backdrop {
      padding: 0;
    }

    .dialog {
      width: 100% !important;
      height: 100%;
      border: none;
      border-radius: 0;
    }
  }
</style>
