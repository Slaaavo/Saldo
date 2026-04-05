import { QueryClient, QueryCache } from '@tanstack/react-query'
import { toast } from 'sonner'
import { extractErrorMessage } from '../shared/utils/errors'

export const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      toast.error(extractErrorMessage(error))
    },
  }),
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
      retry: 0,
    },
  },
})
