<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  type Proc = { pid: number; name: string };

  let processes: Proc[] = $state([]);
  let filter = $state('');
  let loading = $state(false);
  let error = $state('');

  const dispatch = createEventDispatcher<{ status: string }>();

  async function loadProcesses() {
    loading = true;
    error = '';
    try {
      const list = await invoke<Proc[]>('list_processes');
      // Sort by name then pid for stable display
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
      dispatch('status', `Successfully attached to ${name} (PID ${pid})`);
    } catch (e) {
      console.error('Failed to attach:', e);
      dispatch('status', `Failed to attach to ${name} (PID ${pid})`);
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
      class="input w-full p-2"
      placeholder="Filter by name or PID"
      bind:value={filter}
    />
    <button type="button" class="btn preset-tonal" onclick={loadProcesses}>
      Refresh
    </button>
  </div>

  {#if loading}
    <div class="text-sm opacity-70">Loading processes...</div>
  {:else if error}
    <div class="text-sm text-error-500">{error}</div>
  {:else}
    <div class="overflow-auto max-h-64 rounded-md border border-surface-200-800">
      <table class="table w-full text-sm">
        <thead>
          <tr>
            <th class="text-left p-2">Name</th>
            <th class="text-left p-2">PID</th>
            <th class="p-2 w-24"></th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as p}
            <tr class="hover:bg-surface-100-900">
              <td class="p-2">{p.name}</td>
              <td class="p-2">{p.pid}</td>
              <td class="p-2">
                <button type="button" class="btn preset-filled w-full" onclick={() => attach(p.pid, p.name)}>
                  Attach
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>