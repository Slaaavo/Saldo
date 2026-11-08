import type { TableGeometry } from '../fillRange'
import type { DragSurface } from '../useFillDrag'

export const surfaceOf = (scroller: HTMLElement): DragSurface => {
  const box = scroller.getBoundingClientRect()
  const contentLeft = box.left - scroller.scrollLeft
  const columns = [...scroller.querySelectorAll('th[data-month]')].map((th) => th.getBoundingClientRect())
  const edges = columns.map((rect) => rect.left - contentLeft)
  const last = columns[columns.length - 1]
  if (last) edges.push(last.right - contentLeft)
  const label = scroller.querySelector('th[data-label-column]')?.getBoundingClientRect()
  return { edges, stickyWidth: label ? label.right - box.left : 0, scroller }
}

// Offsets ignore transforms, so this stays right while the dialog's zoom-in animation scales every box.
export const layoutOf = (scroller: HTMLElement): TableGeometry => {
  const columns = [...scroller.querySelectorAll<HTMLElement>('th[data-month]')]
  const edges = columns.map((th) => th.offsetLeft)
  const last = columns[columns.length - 1]
  if (last) edges.push(last.offsetLeft + last.offsetWidth)
  return { edges, stickyWidth: scroller.querySelector<HTMLElement>('th[data-label-column]')?.offsetWidth ?? 0 }
}
