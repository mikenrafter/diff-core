/**
 * Static SVG icon markup for the "Open With" editor toolbar buttons.
 *
 * WHY: These are pure data constants that were previously inlined inside the
 * App component function. Moving them here lets CenterPane.tsx import them
 * directly without needing to thread them through React context.
 */
export const editorIcons: Record<string, string> = {
  vscode: `<svg width="16" height="16" viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg"><path d="M180.3 4.5l-56 43.2L59 4.2a8.3 8.3 0 0 0-10.2 1.5L5.1 47.4a8 8 0 0 0 0 11.2L44 96l-39 37.4a8 8 0 0 0 0 11.2l43.7 41.7a8.3 8.3 0 0 0 10.2 1.5l65.3-43.5 56 43.2a12.2 12.2 0 0 0 7 2.5c2 0 4-.5 5.8-1.6l47.5-23a12 12 0 0 0 6.5-10.6V33.2c0-4.4-2.5-8.5-6.5-10.6L193.1 0c-4-2-8.8-1.3-12.8 2.5v2zM192 52.8v150.4L123 128z" fill="#007ACC"/></svg>`,
  cursor: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M1 1l6.5 14L10 9l6-2.5z" stroke="#cdd6f4" stroke-width="1.5" stroke-linejoin="round" fill="none"/><path d="M10 9l4.5 4.5" stroke="#cdd6f4" stroke-width="1.5" stroke-linecap="round"/></svg>`,
  zed: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M2 3h12L2 13h12" stroke="#cdd6f4" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
  vim: `<svg width="16" height="16" viewBox="0 0 544 544" xmlns="http://www.w3.org/2000/svg"><polygon points="272,16 16,272 144,272 272,144 272,272 400,272 528,272 272,16" fill="#019833"/><polygon points="272,528 528,272 400,272 272,400 272,272 144,272 16,272 272,528" fill="#33cc33"/></svg>`,
  terminal: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="1" y="2" width="14" height="12" rx="2" stroke="#cdd6f4" stroke-width="1.2"/><path d="M4 6l2.5 2L4 10" stroke="#a6e3a1" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><path d="M8.5 10H12" stroke="#6c7086" stroke-width="1.2" stroke-linecap="round"/></svg>`,
};
