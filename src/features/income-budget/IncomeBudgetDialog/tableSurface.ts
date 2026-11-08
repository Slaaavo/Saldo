import type { DragSurface } from '../useFillDrag'

/** Measures the month columns (`th[data-month]`) of the table inside `scroller`, in the scrolled content's coordinates. */
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
