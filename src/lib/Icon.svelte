<script lang="ts">
  import mark from "../../assets/brand/tray-template.png";

  const paths = {
    dashboard: ["M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z"],
    table: ["M3 4h18v16H3zM3 9h18M3 14h18M9 4v16"],
    chart: ["M4 3v18h17M8 16v-4m5 4V7m5 9V4"],
    search: ["M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0m-2 5 6 6"],
    columns: ["M3 4h18v16H3zM9 4v16m6-16v16"],
    external: ["M14 3h7v7m0-7L10 14M10 3H3v18h18v-7"],
    up: ["m7 14 5-5 5 5"],
    down: ["m7 10 5 5 5-5"],
    clock: ["M12 8v4l3 2", "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0"],
    calendar: ["M4 6h16v14H4zM4 10h16M8 3v4m8-4v4"],    settings: ["M9 4h6l1 3 3 1 1 5-2 2v3l-5 2-2-2H8l-3-4 1-3-1-3 4-4Z", "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0"],
    left: ["m14 7-5 5 5 5"],
    right: ["m10 7 5 5-5 5"],
    play: ["m9 5 10 7-10 7V5Z"],
    stop: ["M7 7h10v10H7z"],
    close: ["m6 6 12 12M6 18 18 6"],
    plus: ["M12 5v14M5 12h14"],
    download: ["M12 3v12m-5-5 5 5 5-5M5 16v4h14v-4"],
    power: ["M12 3v9M6 6a9 9 0 1 0 12 0"],
    general: ["M4 7h9m4 0h3M4 17h3m4 0h9", "M13 4v6M7 14v6"],
    projects: ["M3 7V5h7l2 3h9v12H3V7Z"],
    bell: ["M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"],
    plug: ["M8 3v4m8-4v4M6 7h12v3a6 6 0 0 1-12 0V7Zm6 9v5"],
    check: ["m5 12 4 4L19 6"],
    copy: ["M8 8h12v13H8zM16 8V3H3v13h5"],
    keyboard: ["M3 6h18v13H3zM7 10h.01M11 10h.01M15 10h.01M18 10h.01M7 14h.01M10 15h7"],
    trash: ["M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"],
  };
  let { name, size = 16 }: { name: keyof typeof paths | "kala"; size?: number } = $props();
</script>

{#if name === "kala"}
  <span class="mark" style:width={`${size}px`} style:height={`${size}px`} style:--mark={`url(${mark})`} aria-hidden="true"></span>
{:else}
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#each paths[name] as d}<path {d} />{/each}
  </svg>
{/if}

<style>
  svg, .mark { display: inline-block; flex-shrink: 0; vertical-align: middle; }
  .mark { background: currentColor; -webkit-mask-image: var(--mark); mask-image: var(--mark); -webkit-mask-size: contain; mask-size: contain; -webkit-mask-position: center; mask-position: center; -webkit-mask-repeat: no-repeat; mask-repeat: no-repeat; }
</style>
