const paths = {
  browse: '<path d="m5 3 14 9-7 1-3 6-4-16Z"/>',
  select:
    '<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/><path d="m9 8 8 5-4 1-2 4-2-10Z"/>',
  draw: '<path d="m4 16 12-12 4 4L8 20H4v-4Zm10-10 4 4"/>',
  parent: '<path d="M12 20V4m-6 6 6-6 6 6"/>',
  collapse: '<path d="m7 7 10 10M17 7 7 17"/>',
  expand: '<path d="M12 5v14M5 12h14"/>',
  send: '<path d="M5 12h14m-6-6 6 6-6 6"/>',
  logo: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3"/>',
};

export function icon(name: keyof typeof paths): string {
  return `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name]}</svg>`;
}
