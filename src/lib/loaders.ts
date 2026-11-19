// Mod loader logos and colours, shared by every place that shows them.

export const LOADER_ICONS: Record<string, string> = {
  vanilla: '/loaders/vanilla.svg',
  fabric: '/loaders/fabric.png',
  quilt: '/loaders/quilt.png',
  neoforge: '/loaders/neoforge.png',
  forge: '/loaders/forge.png'
};

/** Pixel-art logos, drawn sharp instead of blurred when scaled. */
export const PIXEL_ART = new Set(['fabric', 'quilt', 'neoforge']);

/** Each loader's brand colour, used for the glow behind its logo. */
export const LOADER_TINTS: Record<string, string> = {
  vanilla: '#5fa83c',
  fabric: '#dbd0b4',
  quilt: '#9c4bf5',
  neoforge: '#e07a2b',
  forge: '#dfa86a'
};

export const loaderIcon = (loader: string) => LOADER_ICONS[loader] ?? LOADER_ICONS.vanilla;
export const loaderTint = (loader: string) => LOADER_TINTS[loader] ?? LOADER_TINTS.vanilla;
