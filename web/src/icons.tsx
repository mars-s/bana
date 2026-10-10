// Bana's own icon set: simple stroked glyphs on a 16x16 grid, drawn for this project
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
  bell: (
    <>
      <path d="M4.2 11.2V7.6a3.8 3.8 0 0 1 7.6 0v3.6l1.1 1.3H3.1l1.1-1.3Z" />
      <path d="M6.6 13.6a1.5 1.5 0 0 0 2.8 0" />
    </>
  ),
  face: (
    <>
      <circle cx="8" cy="8" r="5.6" />
      <path d="M5.9 9.6c.6.7 1.3 1 2.1 1s1.5-.3 2.1-1M6.2 6.5h.01M9.8 6.5h.01" />
    </>
  ),
  image: (
    <>
      <rect x="2.4" y="3.2" width="11.2" height="9.6" rx="1.8" />
      <circle cx="10.4" cy="6.1" r="1" />
      <path d="m2.8 11.6 3.4-3.4 2.4 2.4 1.5-1.4 2.9 2.6" />
    </>
  ),
  fieldText: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <path d="M7 5.2h2M7 10.8h2M8 5.2v5.6" />
    </>
  ),
  fieldOptions: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <path d="M5.4 5.8h.01M5.4 8h.01M5.4 10.2h.01M7.3 5.8h3.4M7.3 8h3.4M7.3 10.2h3.4" />
    </>
  ),
  fieldDate: (
    <>
      <rect x="2.4" y="3" width="11.2" height="10.6" rx="2" />
      <path d="M2.4 6.2h11.2M5.6 1.9v2.2M10.4 1.9v2.2" />
    </>
  ),
  fieldUser: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <circle cx="8" cy="6.6" r="1.6" />
      <path d="M5.2 11.4c.5-1.3 1.6-2 2.8-2s2.3.7 2.8 2" />
    </>
  ),
  fieldCheck: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <path d="m5.4 8.1 1.8 1.8 3.4-3.6" />
    </>
  ),
  fieldEmail: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <circle cx="8" cy="8" r="1.5" />
      <path d="M9.5 8v.7c0 .7.5 1.1 1 1.1s1-.5 1-1.6A3.5 3.5 0 1 0 9.6 11" />
    </>
  ),
  fieldUrl: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <path d="M7.1 8.9a1.6 1.6 0 0 0 2.2 0l1.2-1.2a1.6 1.6 0 0 0-2.2-2.2l-.4.4M8.9 7.1a1.6 1.6 0 0 0-2.2 0L5.5 8.3a1.6 1.6 0 0 0 2.2 2.2l.4-.4" />
    </>
  ),
  fieldNumber: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <path d="M6.9 5 6.3 11M9.7 5l-.6 6M5.2 6.9h5.6M4.9 9.1h5.6" />
    </>
  ),
  fieldInstance: (
    <>
      <rect x="2.4" y="2.4" width="11.2" height="11.2" rx="2" />
      <circle cx="8" cy="8" r="2.4" />
      <circle cx="8" cy="8" r=".5" fill="currentColor" />
    </>
  ),
  sliders: <path d="M2.8 4.5h10.4M2.8 8h10.4M2.8 11.5h10.4M10 3.3v2.4M5.5 6.8v2.4M9 10.3v2.4" />,
  funnel: <path d="M2.5 4.5h11M4.5 8h7M6.5 11.5h3" />,
  lanes: (
    <>
      <rect x="2.4" y="3" width="11.2" height="10" rx="1.8" />
      <path d="M6.1 3v10M9.9 3v10" />
    </>
  ),
  calendarView: (
    <>
      <rect x="2.4" y="3" width="11.2" height="10.6" rx="2" />
      <path d="M2.4 6.2h11.2M5.6 1.9v2.2M10.4 1.9v2.2" />
      <path d="M5 8.6h.01M8 8.6h.01M11 8.6h.01M5 11h.01M8 11h.01" />
    </>
  ),
  sideMenu: (
    <>
      <rect x="2.4" y="3" width="11.2" height="10" rx="1.8" />
      <path d="M6 3v10M3.8 5.6h1M3.8 7.6h1" />
    </>
  ),
  tabsView: <path d="M2.5 13V5.2c0-.6.4-1 1-1h3.3l1.2 1.6h4.5c.6 0 1 .4 1 1V13a.6.6 0 0 1-.6.6H3.1a.6.6 0 0 1-.6-.6Z" />,
  copy: (
    <>
      <rect x="5.4" y="5.4" width="8" height="8" rx="1.6" />
      <path d="M10.6 5.4V4.2c0-.9-.7-1.6-1.6-1.6H4.2c-.9 0-1.6.7-1.6 1.6V9c0 .9.7 1.6 1.6 1.6h1.2" />
    </>
  ),
  duplicate: (
    <>
      <rect x="2.6" y="2.6" width="8" height="8" rx="1.6" />
      <rect x="5.4" y="5.4" width="8" height="8" rx="1.6" />
    </>
  ),
  globe: (
    <>
      <circle cx="8" cy="8" r="5.6" />
      <path d="M2.4 8h11.2M8 2.4c1.6 1.6 2.3 3.5 2.3 5.6S9.6 12 8 13.6C6.4 12 5.7 10.1 5.7 8S6.4 4 8 2.4Z" />
    </>
  ),
  info: (
    <>
      <circle cx="8" cy="8" r="6" fill="currentColor" stroke="none" />
      <path d="M8 7.4v3.4M8 5.2h.01" stroke="var(--c-on-accent)" />
    </>
  ),
  closeCircle: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="m6 6 4 4M10 6l-4 4" />
    </>
  ),
  play: <path d="M5 3.6v8.8L12.2 8 5 3.6Z" />,
  checkCircle: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="m5.6 8.2 1.7 1.7 3.2-3.4" />
    </>
  ),
  pin: <path d="M6 2.8h4l-.6 3.8 2.1 2.1H4.5l2.1-2.1L6 2.8ZM8 8.7v4.5" />,
  lock: (
    <>
      <rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
      <path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" />
    </>
  ),
  noColor: (
    <>
      <circle cx="8" cy="8" r="5.6" />
      <path d="m4 12 8-8" />
    </>
  ),
  workspace: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M4.8 8h6.4" />
    </>
  ),
  // I-38: an option value's marker (a ring with a dot).
  option: (
    <>
      <circle cx="8" cy="8" r="5.2" />
      <circle cx="8" cy="8" r="1.4" fill="currentColor" stroke="none" />
    </>
  ),
  // I-41 / P06: calendar sync, a generic calendar with two turning arrows (no brand mark).
  calendarSync: (
    <>
      <rect x="2.2" y="3" width="11.6" height="10.6" rx="2" />
      <path d="M2.2 6h11.6M5.2 1.8v2.4M10.8 1.8v2.4" />
      <path d="M5.4 10.4a2.7 2.7 0 0 1 4.6-1.6M10.6 9.2a2.7 2.7 0 0 1-4.6 1.6" />
      <path d="M10.2 7.4v1.5H8.7M5.8 12.2v-1.5h1.5" />
    </>
  ),
  // I-68: a small robot head (model picker).
  robot: (
    <>
      <rect x="3" y="5" width="10" height="8" rx="2.2" />
      <path d="M8 5V2.8" />
      <circle cx="8" cy="2.3" r=".7" fill="currentColor" stroke="none" />
      <circle cx="6" cy="9" r=".8" fill="currentColor" stroke="none" />
      <circle cx="10" cy="9" r=".8" fill="currentColor" stroke="none" />
    </>
  ),
  // I-69: send, a paper plane pointing right.
  send: <path d="M2.5 8 13.5 3l-3.2 10-2.6-4.2L2.5 8Zm5.2.8 5.8-5.8" />,
  // I-70: chats, two overlapping speech bubbles.
  chats: (
    <>
      <path d="M2.5 4.2c0-.9.7-1.6 1.6-1.6h5c.9 0 1.6.7 1.6 1.6v3c0 .9-.7 1.6-1.6 1.6H6l-2 1.8V8.8a1.6 1.6 0 0 1-1.5-1.6v-3Z" />
      <path d="M12 6.6h.3c.7 0 1.2.5 1.2 1.2v2.6c0 .7-.5 1.2-1.2 1.2V13l-1.8-1.4H8.2c-.7 0-1.2-.5-1.2-1.2v-.6" />
    </>
  ),
  // I-71: building blocks, a triangle on two squares.
  blocks: (
    <>
      <path d="M8 2.2 10.8 6.8H5.2L8 2.2Z" />
      <rect x="2.4" y="8.4" width="5" height="5" rx="1" />
      <rect x="8.6" y="8.4" width="5" height="5" rx="1" />
    </>
  ),
  // I-63: AND group toggle, two stacked overlapping squares.
  andGroup: (
    <>
      <rect x="2.5" y="5.5" width="8" height="8" rx="1.6" />
      <path d="M5.5 5.5V4.1c0-.9.7-1.6 1.6-1.6h4.8c.9 0 1.6.7 1.6 1.6v4.8c0 .9-.7 1.6-1.6 1.6h-1.4" />
    </>
  ),
  // I-67: search options, a dot in a soft circle.
  dotCircle: (
    <>
      <circle cx="8" cy="8" r="5.5" />
      <circle cx="8" cy="8" r="2.2" fill="currentColor" stroke="none" />
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
