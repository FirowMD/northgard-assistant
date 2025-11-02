<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshCw, PlugZap, TriangleAlert } from 'lucide-svelte';

  type Proc = { pid: number; name: string };

  let processes: Proc[] = $state([]);
  let filter = $state('');
  let loading = $state(false);
  let error = $state('');

  interface Props {
    onStatus?: (status: string) => void;
  }

  let { onStatus }: Props = $props();

  async function loadProcesses() {
    loading = true;
    error = '';
    try {
      const list = await invoke<Proc[]>('list_processes');
      processes = list.sort((a, b) =>
        a.name.toLowerCase() === b.name.toLowerCase()
          ? a.pid - b.pid
          : a.name.toLowerCase().localeCompare(b.name.toLowerCase())
      );
    } catch (e) {
      console.error('Failed to list processes:', e);
      error = 'Failed to list processes';
    } finally {
      loading = false;
    }
  }

  async function attach(pid: number, name: string) {
    try {
      await invoke('attach_to_pid', { pid });
      onStatus?.(`Successfully attached to ${name} (PID ${pid})`);
    } catch (e) {
      console.error('Failed to attach:', e);
      onStatus?.(`Failed to attach to ${name} (PID ${pid})`);
    }
  }

  const filtered = $derived(
    filter
      ? processes.filter((p) =>
          p.name.toLowerCase().includes(filter.toLowerCase()) ||
          String(p.pid).includes(filter)
        )
      : processes
  );

  onMount(loadProcesses);
</script>

<div class="space-y-2">
  <div class="flex gap-2 items-center">
    <input
      type="text"
      class="input w-full focus:outline-none"
      placeholder="Filter by name or PID"
      bind:value={filter}
    />
    <button
      type="button"
      class="btn h-9 preset-tonal-primary transition-all duration-200 hover:scale-105 active:scale-95"
      onclick={loadProcesses}
      aria-label="Refresh process list"
      title="Refresh process list"
      disabled={loading}
    >
      <RefreshCw size={16} class={loading ? 'animate-spin' : ''} />
    </button>
  </div>

  {#if loading}
    <div class="text-sm opacity-70 animate-pulse" transition:fade={{ duration: 200 }}>
      Loading processes...
    </div>
  {:else if error}
    <div 
      class="preset-filled-error-500 rounded-md p-2 text-sm flex items-center gap-2" 
      transition:fade={{ duration: 300 }}
    >
      <TriangleAlert size={16} />
      <span>{error}</span>
    </div>
  {:else}
    <div class="overflow-auto max-h-64 rounded-lg p-2 preset-tonal-surface" transition:fade={{ duration: 400, delay: 100 }}>
      <table class="table w-full text-sm">
        <thead>
          <tr>
            <th class="text-left p-2">Name</th>
            <th class="text-left p-2">PID</th>
            <th class="p-2 w-24"></th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as p (p.pid)}
            <tr class="hover:bg-surface-100-900 transition-all duration-200 ease-in-out">
              <td class="p-2">{p.name}</td>
              <td class="p-2">{p.pid}</td>
              <td class="p-2">
                <button
                  type="button"
                  class="btn preset-filled-primary-500 w-full flex items-center justify-center gap-2 transition-transform hover:scale-105"
                  onclick={() => attach(p.pid, p.name)}
                  aria-label={`Attach to ${p.name}`}
                  title="Attach"
                >
                  <PlugZap size={16} />
                  <span class="sr-only">Attach</span>
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>