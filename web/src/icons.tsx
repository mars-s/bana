// TanaCraft's own icon set: simple stroked glyphs on a 16x16 grid, drawn for this project
// (no third-party or Tana art). They take the text colour, so they follow the theme.

import type { CSSProperties } from 'react'

const P: Record<string, JSX.Element> = {
  sidebar: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="2" />
      <path d="M6.5 3v10" />
    </>
  ),
  back: <path d="M13 8H3.5M7.5 4 3.5 8l4 4" />,
  forward: <path d="M3 8h9.5M8.5 4l4 4-4 4" />,
  plus: <path d="M8 3v10M3 8h10" />,
  help: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M6.3 6.4a1.8 1.8 0 0 1 3.4.7c0 1.2-1.7 1.4-1.7 2.5" />
      <circle cx="8" cy="11.3" r=".4" fill="currentColor" />
    </>
  ),
  keyboard: (
    <>
      <rect x="1.8" y="4" width="12.4" height="8" rx="1.6" />
      <path d="M4.3 6.6h.01M6.6 6.6h.01M8.9 6.6h.01M11.2 6.6h.01M5.2 9.4h5.6" />
    </>
  ),
  coins: (
    <>
      <ellipse cx="8" cy="6" rx="5" ry="2.2" />
      <path d="M3 6v3.6C3 10.8 5.2 12 8 12s5-1.2 5-2.4V6" />
    </>
  ),
  user: (
    <>
      <circle cx="8" cy="5.5" r="2.6" />
      <path d="M3 13.5c.6-2.4 2.6-3.6 5-3.6s4.4 1.2 5 3.6" />
    </>
  ),
  calendar: (
    <>
      <rect x="2.5" y="3.2" width="11" height="10.3" rx="1.8" />
      <path d="M2.5 6.2h11M5.5 2v2.4M10.5 2v2.4" />
    </>
  ),
  hash: <path d="M6.3 2.5 5 13.5M11 2.5 9.7 13.5M3 6h10.5M2.5 10H13" />,
  clock: (
    <>
      <path d="M2.6 8a5.4 5.4 0 1 0 1.6-3.8" />
      <path d="M2.4 2.6v2.6H5M8 5v3.2l2 1.3" />
    </>
  ),
  sparkle: (
    <>
      <path d="M6.5 2.5c.4 2.6 1.4 3.6 4 4-2.6.4-3.6 1.4-4 4-.4-2.6-1.4-3.6-4-4 2.6-.4 3.6-1.4 4-4Z" />
      <path d="M12 9.5c.2 1.2.7 1.7 1.9 1.9-1.2.2-1.7.7-1.9 1.9-.2-1.2-.7-1.7-1.9-1.9 1.2-.2 1.7-.7 1.9-1.9Z" />
    </>
  ),
  compose: (
    <>
      <path d="M13 8.6V12a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 3 12V5a1.5 1.5 0 0 1 1.5-1.5H8" />
      <path d="m11.4 2.6 2 2L8.3 9.7l-2.6.6.6-2.6 5.1-5.1Z" />
    </>
  ),
  search: (
    <>
      <circle cx="7" cy="7" r="4.2" />
      <path d="m10.2 10.2 3.3 3.3" />
    </>
  ),
  chevronLeft: <path d="M10 3.5 5.5 8l4.5 4.5" />,
  chevronRight: <path d="M6 3.5 10.5 8 6 12.5" />,
  chevronDown: <path d="M3.5 6 8 10.5 12.5 6" />,
  width: <path d="M2.5 3v10M13.5 3v10M5 8h6M7 6l-2 2 2 2M9 6l2 2-2 2" />,
  more: (
    <>
      <circle cx="3.6" cy="8" r=".9" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r=".9" fill="currentColor" stroke="none" />
      <circle cx="12.4" cy="8" r=".9" fill="currentColor" stroke="none" />
    </>
  ),
  close: <path d="m4 4 8 8M12 4l-8 8" />,
  check: <path d="m3.5 8.4 2.9 2.8 6-6.4" />,
  external: <path d="M9.5 2.8h3.7v3.7M13 3 7.5 8.5M11.5 9.5v2.8a.9.9 0 0 1-.9.9H3.7a.9.9 0 0 1-.9-.9V5.4c0-.5.4-.9.9-.9h2.8" />,
  trash: <path d="M3 4.5h10M6.2 4.5V3h3.6v1.5M4.3 4.5l.6 8.3c0 .4.4.7.8.7h4.6c.4 0 .8-.3.8-.7l.6-8.3" />,
  home: <path d="M2.8 7.2 8 2.8l5.2 4.4M4.3 6v7.2h7.4V6" />,
  gear: (
    <>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 1.8v1.8M8 12.4v1.8M1.8 8h1.8M12.4 8h1.8M3.6 3.6l1.3 1.3M11.1 11.1l1.3 1.3M3.6 12.4l1.3-1.3M11.1 4.9l1.3-1.3" />
    </>
  ),
  inbox: <path d="M2.5 9.5 4 3.5h8l1.5 6v3h-11v-3ZM2.5 9.5h3.2l.8 1.5h3l.8-1.5h3.2" />,
  daily: (
    <>
      <rect x="2.5" y="3.2" width="11" height="10.3" rx="1.8" />
      <path d="M2.5 6.2h11M5.5 9h1M7.5 9h1M9.5 9h1M5.5 11h1M7.5 11h1" />
    </>
  ),
  page: <path d="M4 2.5h5l3 3v8H4v-11ZM9 2.5v3h3" />,
  list: <path d="M6 4.5h7M6 8h7M6 11.5h7M3 4.5h.01M3 8h.01M3 11.5h.01" />,
  table: (
    <>
      <rect x="2.5" y="3" width="11" height="10" rx="1.4" />
      <path d="M2.5 6.5h11M2.5 10h11M6.5 6.5V13" />
    </>
  ),
  cards: (
    <>
      <rect x="2.5" y="2.8" width="4.6" height="4.6" rx="1" />
      <rect x="8.9" y="2.8" width="4.6" height="4.6" rx="1" />
      <rect x="2.5" y="8.6" width="4.6" height="4.6" rx="1" />
      <rect x="8.9" y="8.6" width="4.6" height="4.6" rx="1" />
    </>
  ),
  filter: <path d="M2.5 3.5h11L9.2 8.6v4l-2.4 1V8.6L2.5 3.5Z" />,
  sort: <path d="M5 2.8v10.4M2.8 5 5 2.8 7.2 5M11 13.2V2.8M8.8 11l2.2 2.2 2.2-2.2" />,
  group: <path d="M2.5 3.5h11M2.5 6.5h7M2.5 9.5h11M2.5 12.5h7" />,
  mic: (
    <>
      <rect x="6" y="2.3" width="4" height="7" rx="2" />
      <path d="M3.8 7.8a4.2 4.2 0 0 0 8.4 0M8 12v1.8" />
    </>
  ),
  board: (
    <>
      <rect x="2.5" y="3" width="3.2" height="10" rx=".8" />
      <rect x="6.4" y="3" width="3.2" height="7" rx=".8" />
      <rect x="10.3" y="3" width="3.2" height="8.5" rx=".8" />
    </>
  ),
  field: (
    <>
      <rect x="2.5" y="3" width="11" height="10" rx="1.6" />
      <path d="M5.5 3v10" />
    </>
  ),
  link: <path d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2-2a2.6 2.6 0 0 0-3.7-3.7l-.6.6M9.2 6.8a2.6 2.6 0 0 0-3.7 0l-2 2a2.6 2.6 0 0 0 3.7 3.7l.6-.6" />,
  workspace: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M4.8 8h6.4" />
    </>
  ),
}

export type IconName = keyof typeof P

export function Icon({ name, size = 16, style, className }: { name: IconName; size?: number; style?: CSSProperties; className?: string }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.35}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={style}
      aria-hidden="true"
    >
      {P[name]}
    </svg>
  )
}

/** The Today icon: a calendar page showing the day number. */
export function TodayIcon({ day }: { day: number }) {
  return (
    <svg width={14} height={14} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.35} aria-hidden="true">
      <rect x="2.3" y="2.6" width="11.4" height="11" rx="2" />
      <path d="M2.3 5.4h11.4" />
      <text x="8" y="12.2" textAnchor="middle" fontSize="6.2" fontWeight="600" fill="currentColor" stroke="none" fontFamily="var(--font)">
        {day}
      </text>
    </svg>
  )
}
