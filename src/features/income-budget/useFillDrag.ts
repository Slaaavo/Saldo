import { useEffect, useEffectEvent, useRef, useState } from 'react'
import type { DraftRow } from './budgetDraft'
import { columnAtX, edgeScrollDelta, type FillSpan, type TableGeometry } from './fillRange'

export interface DragSurface extends TableGeometry {
  scroller: { scrollLeft: number; readonly clientWidth: number; getBoundingClientRect(): { left: number } }
}

export interface FillPointerDown {
  button: number
  pointerId: number
  currentTarget: { setPointerCapture(pointerId: number): void }
  preventDefault(): void
}

interface Session {
  span: FillSpan
  surface: DragSurface
  viewportX: number
}

interface Options {
  measure: () => DragSurface | null
  onFill: (span: FillSpan) => void
}

export const useFillDrag = ({ measure, onFill }: Options) => {
  const [drag, setDrag] = useState<FillSpan | null>(null)
  const session = useRef<Session | null>(null)
  const fill = useEffectEvent(onFill)

  const start = (row: DraftRow, originIdx: number, event: FillPointerDown) => {
    if (event.button !== 0) return
    const surface = measure()
    if (!surface) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    const span = { row, originIdx, targetIdx: originIdx }
    session.current = { span, surface, viewportX: 0 }
    setDrag(span)
  }

  const active = drag !== null
  useEffect(() => {
    const current = session.current
    if (!active || !current) return
    const { surface } = current
    const { scroller } = surface
    let frame: number | null = null

    const pointer = () => ({ viewportX: current.viewportX, scrollLeft: scroller.scrollLeft, viewportWidth: scroller.clientWidth })
    const retarget = () => {
      const targetIdx = columnAtX(surface, pointer())
      if (targetIdx === current.span.targetIdx) return
      current.span = { ...current.span, targetIdx }
      setDrag(current.span)
    }
    const scrollStep = () => {
      frame = null
      const delta = edgeScrollDelta(surface, pointer())
      if (delta === 0) return
      const before = scroller.scrollLeft
      scroller.scrollLeft = before + delta
      if (scroller.scrollLeft === before) return
      retarget()
      frame = requestAnimationFrame(scrollStep)
    }
    const move = (event: PointerEvent) => {
      current.viewportX = event.clientX - scroller.getBoundingClientRect().left
      retarget()
      if (frame === null) scrollStep()
    }
    const finish = (commit: boolean) => {
      session.current = null
      setDrag(null)
      if (commit && current.span.targetIdx !== current.span.originIdx) fill(current.span)
    }
    const release = () => finish(true)
    const cancel = () => finish(false)
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      cancel()
    }

    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', release)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', escape, true)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', release)
      window.removeEventListener('pointercancel', cancel)
      window.removeEventListener('keydown', escape, true)
      if (frame !== null) cancelAnimationFrame(frame)
    }
  }, [active])

  return { drag, start }
}
