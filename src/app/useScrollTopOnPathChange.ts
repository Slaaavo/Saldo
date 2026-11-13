import { useEffect, useRef } from 'react'

export const useScrollTopOnPathChange = (path: string) => {
  const scrollRef = useRef<HTMLElement>(null)

  useEffect(() => {
    // WKWebView keeps painting the old offset when the scroll position changes in the frame that swaps the page's content.
    const frame = requestAnimationFrame(() => {
      if (scrollRef.current) scrollRef.current.scrollTop = 0
    })
    return () => cancelAnimationFrame(frame)
  }, [path])

  return scrollRef
}
