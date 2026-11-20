// Iconos propios (trazo 1.8, esquinas redondeadas): consistentes y sin dependencia externa.
import type { JSX } from "preact";

type P = { size?: number; class?: string; title?: string };

const I = (paths: JSX.Element, fill = false) =>
  ({ size = 18, class: cls, title }: P) => (
    <svg
      class={cls}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke={fill ? "none" : "currentColor"}
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden={title ? undefined : "true"}
      role={title ? "img" : undefined}
    >
      {title && <title>{title}</title>}
      {paths}
    </svg>
  );

export const Search = I(<><circle cx="11" cy="11" r="6.5" /><path d="m20 20-4.2-4.2" /></>);
export const Sparkle = I(<path d="M12 3c.6 4.2 1.8 5.4 6 6-4.2.6-5.4 1.8-6 6-.6-4.2-1.8-5.4-6-6 4.2-.6 5.4-1.8 6-6ZM18.5 15.5c.3 1.6.9 2.2 2.5 2.5-1.6.3-2.2.9-2.5 2.5-.3-1.6-.9-2.2-2.5-2.5 1.6-.3 2.2-.9 2.5-2.5Z" />);
export const Gif = I(<><rect x="3" y="5" width="18" height="14" rx="3.5" /><path d="M10 10H8.5A1.5 1.5 0 0 0 7 11.5v1A1.5 1.5 0 0 0 8.5 14H10v-2M13 10v4M16 14v-4h2M16 12h1.5" /></>);
export const Film = I(<><rect x="3" y="4" width="18" height="16" rx="3.5" /><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4" /></>);
export const Image = I(<><rect x="3" y="4" width="18" height="16" rx="3.5" /><circle cx="9" cy="10" r="1.6" /><path d="m21 16-4.5-4.5L7 20" /></>);
export const Music = I(<><path d="M9 17V6l11-2v11" /><circle cx="6.5" cy="17" r="2.5" /><circle cx="17.5" cy="15" r="2.5" /></>);
export const Laugh = I(<><circle cx="12" cy="12" r="9" /><path d="M8 14.5a4.5 4.5 0 0 0 8 0Z" /><path d="M8.5 9.5h.01M15.5 9.5h.01" /></>);
export const Heart = I(<path d="M12 20s-7.5-4.4-7.5-10A4.3 4.3 0 0 1 12 7.6 4.3 4.3 0 0 1 19.5 10c0 5.6-7.5 10-7.5 10Z" />);
export const HeartFill = I(<path d="M12 20.6s-8.2-4.8-8.2-10.9A4.9 4.9 0 0 1 12 6.6a4.9 4.9 0 0 1 8.2 3.1c0 6.1-8.2 10.9-8.2 10.9Z" />, true);
export const Stack = I(<><path d="m12 3 9 4.5-9 4.5-9-4.5L12 3Z" /><path d="m3 12 9 4.5 9-4.5M3 16.5 12 21l9-4.5" /></>);
export const Clock = I(<><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>);
export const Download = I(<><path d="M12 4v11M7 10.5l5 5 5-5" /><path d="M5 19.5h14" /></>);
export const Sliders = I(<><path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2" /><circle cx="10" cy="17" r="2" /></>);
export const Pulse = I(<path d="M3 12h4l2.5-6 4 12 2.5-6H21" />);
export const Play = I(<path d="M7.5 5.3c0-.9 1-1.5 1.8-1l10 6.2c.7.5.7 1.5 0 2l-10 6.2c-.8.5-1.8-.1-1.8-1V5.3Z" />, true);
export const Pause = I(<><rect x="6" y="5" width="4" height="14" rx="1.3" /><rect x="14" y="5" width="4" height="14" rx="1.3" /></>, true);
export const Volume = I(<><path d="M4 10v4h3.5L12 18V6L7.5 10H4Z" /><path d="M16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" /></>);
export const Mute = I(<><path d="M4 10v4h3.5L12 18V6L7.5 10H4Z" /><path d="m16 10 4 4M20 10l-4 4" /></>);
export const Repeat = I(<><path d="M17 3l3 3-3 3" /><path d="M20 6H8a4 4 0 0 0-4 4v1M7 21l-3-3 3-3" /><path d="M4 18h12a4 4 0 0 0 4-4v-1" /></>);
export const Expand = I(<path d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4" />);
export const Close = I(<path d="M6 6l12 12M18 6 6 18" />);
export const Plus = I(<path d="M12 5v14M5 12h14" />);
export const External = I(<><path d="M14 4h6v6M20 4l-9 9" /><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4" /></>);
export const Copy = I(<><rect x="8" y="8" width="12" height="12" rx="2.5" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></>);
export const Check = I(<path d="m5 12.5 4.5 4.5L19 7.5" />);
export const Alert = I(<><path d="M12 4 2.8 19.5h18.4L12 4Z" /><path d="M12 10v4M12 17h.01" /></>);
export const Trash = I(<><path d="M4 7h16M10 11v6M14 11v6" /><path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" /></>);
export const Folder = I(<path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h3.5l2 2.5H18A2.5 2.5 0 0 1 20.5 10v7A2.5 2.5 0 0 1 18 19.5H6A2.5 2.5 0 0 1 3.5 17V7.5Z" />);
export const Refresh = I(<><path d="M20 11a8 8 0 0 0-14.6-4.5L4 8" /><path d="M4 4v4h4M4 13a8 8 0 0 0 14.6 4.5L20 16" /><path d="M20 20v-4h-4" /></>);
export const Pencil = I(<path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z" />);
export const Key = I(<><circle cx="8" cy="15" r="4" /><path d="m11 12 8-8M16 7l2 2M14 9l2 2" /></>);
export const Globe = I(<><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></>);
