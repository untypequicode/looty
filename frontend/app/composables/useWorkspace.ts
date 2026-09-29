import { ref, computed } from 'vue'
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query'
import { useSessionStorage } from '@vueuse/core'
import type { Workspace, WorkspaceMember, Page, Board } from '../types'

export const useWorkspace = () => {
  const queryClient = useQueryClient()
  const activeWorkspaceId = useSessionStorage<string | null>('looty_active_workspace', null)

  const { data: workspacesData, isLoading: isLoadingWorkspaces } = useQuery({
    queryKey: ['workspaces'],
    queryFn: async () => {
      const response = await $fetch('/api/workspaces')
      return response as { workspaces: Workspace[] }
    }
  })

  const { data: workspaceMembers, isLoading: isLoadingMembers } = useQuery({
    queryKey: ['workspaces', activeWorkspaceId, 'members'],
    queryFn: async () => {
      if (!activeWorkspaceId.value) return []
      const response = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/members`) as { members: WorkspaceMember[] }
      return response.members
    },
    enabled: computed(() => !!activeWorkspaceId.value)
  })

  const { data: pages, isLoading: isLoadingPages } = useQuery({
    queryKey: ['workspaces', activeWorkspaceId, 'pages'],
    queryFn: async () => {
      if (!activeWorkspaceId.value) return []
      const response = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/pages`)
      return response as Page[]
    },
    enabled: computed(() => !!activeWorkspaceId.value)
  })
  
  const { data: boards, isLoading: isLoadingBoards } = useQuery({
    queryKey: ['workspaces', activeWorkspaceId, 'boards'],
    queryFn: async () => {
      if (!activeWorkspaceId.value) return []
      const response = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/boards`)
      return response as Board[]
    },
    enabled: computed(() => !!activeWorkspaceId.value)
  })

  const { data: myTasks, isLoading: isLoadingMyTasks } = useQuery({
    queryKey: ['workspaces', activeWorkspaceId, 'my_tasks'],
    queryFn: async () => {
      if (!activeWorkspaceId.value) return []
      const response = await $fetch(`/api/workspaces/${activeWorkspaceId.value}/my_tasks`) as { tasks: any[] }
      return response.tasks
    },
    enabled: computed(() => !!activeWorkspaceId.value)
  })

  const activeWorkspaceRole = computed(() => {
    const ws = workspacesData.value?.workspaces?.find((w: Workspace) => w.id === activeWorkspaceId.value)
    return ws ? ws.role : null
  })

  const selectWorkspace = (id: string | null) => {
    activeWorkspaceId.value = id
  }

  // Mutations
  const createWorkspaceMutation = useMutation({
    mutationFn: async (name: string) => {
      return await $fetch('/api/workspaces', {
        method: 'POST',
        body: { name }
      }) as Workspace
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
    }
  })

  const renameWorkspaceMutation = useMutation({
    mutationFn: async ({ id, name }: { id: string, name: string }) => {
      return await $fetch(`/api/workspaces/${id}`, {
        method: 'PUT',
        body: { name }
      }) as Workspace
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
    }
  })

  const deleteWorkspaceMutation = useMutation({
    mutationFn: async (id: string) => {
      return await $fetch(`/api/workspaces/${id}`, {
        method: 'DELETE'
      })
    },
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
      if (activeWorkspaceId.value === id) {
        activeWorkspaceId.value = null
      }
    }
  })

  const createPageMutation = useMutation({
    mutationFn: async ({ workspaceId, parentId }: { workspaceId: string, parentId?: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/pages`, {
        method: 'POST',
        body: { title: 'Nouvelle page', parent_id: parentId || null, content: '' }
      }) as Page
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'pages'] })
    }
  })
  
  const deletePageMutation = useMutation({
    mutationFn: async ({ workspaceId, pageId }: { workspaceId: string, pageId: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/pages/${pageId}`, {
        method: 'DELETE'
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'pages'] })
    }
  })

  const updatePageMutation = useMutation({
    mutationFn: async ({ workspaceId, pageId, data }: { workspaceId: string, pageId: string, data: Partial<Page> }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/pages/${pageId}`, {
        method: 'PUT',
        body: data
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'pages'] })
    }
  })

  const sharePageMutation = useMutation({
    mutationFn: async ({ workspaceId, pageId }: { workspaceId: string, pageId: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/pages/${pageId}/share`, {
        method: 'POST'
      }) as Page
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'pages'] })
    }
  })

  const revokeShareMutation = useMutation({
    mutationFn: async ({ workspaceId, pageId }: { workspaceId: string, pageId: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/pages/${pageId}/share`, {
        method: 'DELETE'
      }) as Page
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'pages'] })
    }
  })

  const changeMemberRoleMutation = useMutation({
    mutationFn: async ({ workspaceId, memberId, role }: { workspaceId: string, memberId: string, role: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/members/${memberId}`, {
        method: 'PUT',
        body: { role }
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'members'] })
    }
  })

  const kickMemberMutation = useMutation({
    mutationFn: async ({ workspaceId, memberId }: { workspaceId: string, memberId: string }) => {
      return await $fetch(`/api/workspaces/${workspaceId}/members/${memberId}`, {
        method: 'DELETE'
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', activeWorkspaceId, 'members'] })
    }
  })

  const copyInviteLink = (workspaceId: string) => {
    const url = `${window.location.origin}/invite/${workspaceId}`
    navigator.clipboard.writeText(url)
    alert("Lien d'invitation copié : " + url)
  }

  return {
    activeWorkspaceId,
    activeWorkspaceRole,
    workspacesData,
    isLoadingWorkspaces,
    workspaceMembers,
    isLoadingMembers,
    pages,
    isLoadingPages,
    boards,
    isLoadingBoards,
    myTasks,
    isLoadingMyTasks,
    selectWorkspace,
    createWorkspace: createWorkspaceMutation.mutateAsync,
    isCreatingWorkspace: createWorkspaceMutation.isPending,
    renameWorkspace: renameWorkspaceMutation.mutateAsync,
    deleteWorkspace: deleteWorkspaceMutation.mutateAsync,
    createPage: createPageMutation.mutateAsync,
    updatePage: updatePageMutation.mutateAsync,
    deletePage: deletePageMutation.mutateAsync,
    sharePage: sharePageMutation.mutateAsync,
    revokeShare: revokeShareMutation.mutateAsync,
    changeMemberRole: changeMemberRoleMutation.mutateAsync,
    kickMember: kickMemberMutation.mutateAsync,
    copyInviteLink
  }
}
