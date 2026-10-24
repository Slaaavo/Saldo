export const hasErrorCode = (err: unknown, code: string): boolean => {
  return typeof err === 'object' && err !== null && 'code' in err && (err as { code: unknown }).code === code
}

export const extractErrorMessage = (err: unknown): string => {
  if (typeof err === 'object' && err !== null && 'message' in err && typeof (err as { message: unknown }).message === 'string') {
    return (err as { message: string }).message
  }
  return String(err)
}
