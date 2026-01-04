<script>
  import { createEventDispatcher } from 'svelte'

  export let results = []
  export let active = 'bad'

  const dispatch = createEventDispatcher()

  $: total = results.length
  $: bad = results.filter((r) => r.status === 'bad').length
  $: ok = results.filter((r) => r.status === 'ok').length
  $: err = results.filter((r) => r.status === 'error').length

  function setFilter(val) {
    dispatch('filter', val)
  }
</script>

<div class="scan-summary">
  <button class={`pill ghost ${active === 'all' ? 'active' : ''}`} on:click={() => setFilter('all')}>
    Total {total}
  </button>
  <button class={`pill warn ${active === 'bad' ? 'active' : ''}`} on:click={() => setFilter('bad')}>
    Low {bad}
  </button>
  <button class={`pill ${active === 'ok' ? 'active' : ''}`} on:click={() => setFilter('ok')}>
    OK {ok}
  </button>
  <button class={`pill ${active === 'error' ? 'active' : ''}`} on:click={() => setFilter('error')}>
    Err {err}
  </button>
</div>

<style>
  .scan-summary {
    display: flex;
    gap: 8px;
  }
  .pill.active {
    outline: 2px solid currentColor;
    box-shadow: 0 0 0 2px rgba(0,0,0,0.08);
    transform: translateY(-1px);
  }
  .pill.warn.active {
    outline-color: var(--accent);
  }
</style>
