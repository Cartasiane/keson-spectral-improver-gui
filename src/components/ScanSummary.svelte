<script>
  import { createEventDispatcher } from "svelte";
  import { Check } from "lucide-svelte";

  export let results = [];
  export let active = "bad";
  export let downloadedCount = 0;

  const dispatch = createEventDispatcher();

  $: total = results.length;
  $: bad = results.filter((r) => r.status === "bad").length;
  $: ok = results.filter((r) => r.status === "ok").length;
  $: err = results.filter((r) => r.status === "error").length;

  function setFilter(val) {
    dispatch("filter", val);
  }
</script>

<div class="scan-summary">
  <button
    class={`pill ghost ${active === "all" ? "active" : ""}`}
    on:click={() => setFilter("all")}
  >
    Total {total}
  </button>
  <button
    class={`pill warn ${active === "bad" ? "active" : ""}`}
    on:click={() => setFilter("bad")}
  >
    Low {bad}
  </button>
  <button
    class={`pill ${active === "ok" ? "active" : ""}`}
    on:click={() => setFilter("ok")}
  >
    OK {ok}
  </button>
  {#if downloadedCount > 0}
    <button
      class={`pill success ${active === "downloaded" ? "active" : ""}`}
      on:click={() => setFilter("downloaded")}
    >
      <Check size={16} /> Downloaded {downloadedCount}
    </button>
  {/if}
  <button
    class={`pill ${active === "error" ? "active" : ""}`}
    on:click={() => setFilter("error")}
  >
    Err {err}
  </button>
</div>

<style>
  .scan-summary {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .pill.active {
    outline: 2px solid currentColor;
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.08);
    transform: translateY(-1px);
  }
  .pill.warn.active {
    outline-color: var(--accent);
  }
  .pill.success {
    background: linear-gradient(135deg, #10b981, #059669);
    color: white;
  }
  .pill.success.active {
    outline-color: #10b981;
  }
</style>
