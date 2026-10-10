// The one popover layer: every menu, picker and popover renders here, in a portal on <body>, so no
// panel, table or scroller (overflow, transform, z-index) can clip or cover it. It is placed next
// to its anchor (below, start-aligned by default), flips above when there is more room there, and
// shifts to stay inside the window.

import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'

export interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** An element (its box is read when placing) or a fixed box, e.g. a point `{x, y, x, y}`. */
export type Anchor = RefObject<HTMLElement | null> | Box

/** Room kept between a popover and the window's edges. */
export const EDGE = 8
/** Gap between an anchor and its popover (--q1). */
const GAP = 3.75

export function pointBox(x: number, y: number): Box {
  return { left: x, top: y, right: x, bottom: y }
}

/** Where a `w` x `h` box goes by anchor `a` in a `vw` x `vh` window: under it (or above it when
 * there is no room below and there is above), its left edge on the anchor's (or its right edge,
 * `end`), then moved inside the window. Taller than the window: it scrolls (`maxHeight`). */
export function place(a: Box, w: number, h: number, vw: number, vh: number, align: 'start' | 'end' | 'side' = 'start', gap = GAP) {
  let top: number
  let left: number
  if (align === 'side') {
    // A submenu: to the right of its item, or to the left when only that fits.
    top = a.top
    left = a.right + gap
    if (left + w > vw - EDGE && a.left - gap - w >= EDGE) left = a.left - gap - w
  } else {
    top = a.bottom + gap
    if (top + h > vh - EDGE && a.top - gap - h >= EDGE) top = a.top - gap - h
    left = align === 'end' ? a.right - w : a.left
  }
  top = Math.max(EDGE, Math.min(top, vh - h - EDGE))
  left = Math.max(EDGE, Math.min(left, vw - w - EDGE))
  const maxHeight = h > vh - 2 * EDGE ? vh - 2 * EDGE : undefined
  return { left, top, maxHeight }
}

function boxOf(anchor: Anchor): Box | undefined {
  return 'current' in anchor ? anchor.current?.getBoundingClientRect() : anchor
}

export function Popover({
  anchor,
  align = 'start',
  gap,
  className = 'menu pop',
  style,
  onClose,
  children,
  ...rest
}: {
  anchor: Anchor
  align?: 'start' | 'end' | 'side'
  gap?: number
  className?: string
  style?: CSSProperties
  /** Called on a mouse press outside the popover and its anchor element, and on Escape. */
  onClose?: () => void
  children: ReactNode
  role?: string
  'aria-label'?: string
  onMouseDown?: (e: React.MouseEvent) => void
  onClick?: (e: React.MouseEvent) => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<ReturnType<typeof place> | null>(null)
  const capped = useRef(false)
  const box = 'current' in anchor ? null : anchor
  const key = box ? `${box.left},${box.top},${box.right},${box.bottom}` : anchor
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const update = () => {
      const a = boxOf(anchor)
      if (!a) return
      // Once capped to the window it scrolls, and its content height says whether it still must.
      const h = capped.current ? el.scrollHeight : el.offsetHeight
      const next = place(a, el.offsetWidth, h, window.innerWidth, window.innerHeight, align, gap)
      capped.current = next.maxHeight !== undefined
      setPos((p) => (p && p.left === next.left && p.top === next.top && p.maxHeight === next.maxHeight ? p : next))
    }
    update()
    // Content that grows (a filtered list), a resized window or a scrolled panel moves it again.
    const ro = new ResizeObserver(update)
    ro.observe(el)
    window.addEventListener('resize', update)
    window.addEventListener('scroll', update, true)
    return () => {
      ro.disconnect()
      window.removeEventListener('resize', update)
      window.removeEventListener('scroll', update, true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, align, gap])
  const close = useRef(onClose)
  close.current = onClose
  const dismissable = !!onClose
  useEffect(() => {
    if (!dismissable) return
    const down = (e: MouseEvent) => {
      const t = e.target as Node
      const a = 'current' in anchor ? anchor.current : null
      const inSub = (t as Element).closest?.('.menu.submenu')
      if (!ref.current?.contains(t) && !a?.contains(t) && !inSub) close.current?.()
    }
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (e.stopPropagation(), close.current?.())
    window.addEventListener('mousedown', down)
    window.addEventListener('keydown', esc, true)
    return () => {
      window.removeEventListener('mousedown', down)
      window.removeEventListener('keydown', esc, true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dismissable, key])
  const placed: CSSProperties = pos
    ? { position: 'fixed', left: pos.left, top: pos.top, ...(pos.maxHeight ? { maxHeight: pos.maxHeight, overflowY: 'auto' } : {}) }
    : { position: 'fixed', left: 0, top: 0 } // moved before the first paint; visible so autoFocus works
  return createPortal(
    <div ref={ref} className={className} style={{ ...style, ...placed }} {...rest}>
      {children}
    </div>,
    document.body,
  )
}
