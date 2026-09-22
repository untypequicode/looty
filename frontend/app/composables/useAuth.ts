import { useQuery } from '@tanstack/vue-query'

export const useAuth = () => {
  const { data: user, isLoading, isError } = useQuery({
    queryKey: ['auth', 'me'],
    queryFn: async () => {
      const response = await $fetch('/api/auth/me')
      return response as { authenticated: boolean; user?: any }
    },
    staleTime: 5 * 60 * 1000, // Cache de 5 minutes
    retry: false
  })

  return { user, isLoading, isError }
}
