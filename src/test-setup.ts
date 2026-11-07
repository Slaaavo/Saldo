import '@testing-library/jest-dom'

const noop = () => {}

globalThis.ResizeObserver ??= class {
  observe = noop
  unobserve = noop
  disconnect = noop
}
