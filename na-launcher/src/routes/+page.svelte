<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Accordion } from '@skeletonlabs/skeleton-svelte';
  import ProcessAttacher from '../lib/ProcessAttacher.svelte';
  import { Play, Info } from 'lucide-svelte';

  let displayText = $state("Launcher is up-to-date");
  const value = $state(['advanced']);

  async function handlePlay() {
    try {
      await invoke("launch_northgard");
    } catch (e) {
      console.error("Failed to start game:", e);
    }
  }
</script>


<main class="h-screen p-6">
  <div class="mx-auto max-w-[720px] space-y-6">
    <div class="flex items-center justify-between preset-filled-primary-500 rounded-xl p-2">
      <div class="flex items-center gap-2">
        <Info class="opacity-80" size={18} />
        <span class="text-sm">{displayText}</span>
      </div>
      <button 
        type="button"
        class="btn preset-filled-primary-500 flex items-center gap-2"
        onclick={handlePlay}
        aria-label="Play Northgard"
        title="Launch Northgard"
      >
        <Play size={18} />
        <span class="sr-only">Play</span>
      </button>
    </div>

    <Accordion collapsible>
      <Accordion.Item value="advanced">
        <h3>
          <Accordion.ItemTrigger class="flex justify-between items-center rounded-lg p-2">
            <span class="font-medium">Advanced</span>
          </Accordion.ItemTrigger>
        </h3>
        <Accordion.ItemContent class="preset-tonal-surface rounded-lg p-3">
          <div class="space-y-2">
            <ProcessAttacher onStatus={(status) => (displayText = status)} />
          </div>
        </Accordion.ItemContent>
      </Accordion.Item>
    </Accordion>
  </div>
</main>