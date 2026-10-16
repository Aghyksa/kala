import { i18n } from "./i18n.svelte";

export const wt = (english: string, indonesian: string): string =>
  i18n.lang === "id" ? indonesian : english;
