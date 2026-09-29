<template>
  <div class="public-page-container">
    <div v-if="pending" class="loading-state">
      Chargement de la page...
    </div>
    <div v-else-if="error" class="error-state">
      <h2>Page introuvable</h2>
      <p>Cette page n'existe pas ou n'est plus partagée publiquement.</p>
    </div>
    <div v-else-if="page" class="public-page-content">
      <div class="page-header">
        <div class="page-title-row">
          <div class="page-icon readonly" v-if="page.icon">{{ page.icon }}</div>
          <h1 class="page-title">{{ page.title }}</h1>
        </div>
        <div class="page-meta">
          <span class="meta-readonly">Lecture seule (Lien public)</span>
        </div>
      </div>
      <div class="page-body">
        <WysiwygEditor 
          :key="page.id"
          :modelValue="page.content || ''" 
          :document-id="page.id"
          :editable="false"
          :isPublic="true"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRoute } from 'vue-router'
import WysiwygEditor from '~/components/WysiwygEditor.client.vue'
import { computed } from 'vue'

const route = useRoute()
const token = route.params.token as string

const { data: page, pending, error } = useFetch(`/api/public/pages/${token}`)

// We don't have real-time collaborative editing on public view (or maybe read-only).
// We set editable to false.

// Update head for SEO
useHead(computed(() => ({
  title: page.value ? `${page.value.title} - Looty` : 'Chargement... - Looty',
})))
</script>

<style scoped>
.public-page-container {
  min-height: 100vh;
  background: var(--bg-color);
  color: var(--text-color);
  padding: 48px 24px;
  display: flex;
  justify-content: center;
}

.public-page-content {
  width: 100%;
  max-width: 900px;
  background: var(--bg-color-2);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: 48px;
  box-shadow: var(--shadow-md);
}

.page-header {
  margin-bottom: 32px;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 24px;
}

.page-title-row {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 12px;
}

.page-icon {
  font-size: 2.5rem;
}

.page-title {
  font-size: 2.5rem;
  font-weight: 700;
  margin: 0;
}

.page-meta {
  color: var(--text-color-3);
  font-size: 0.9rem;
}

.meta-readonly {
  background: var(--primary-color-alpha);
  color: var(--primary-color);
  padding: 4px 8px;
  border-radius: 4px;
  font-weight: 600;
}

.loading-state, .error-state {
  text-align: center;
  margin-top: 100px;
  color: var(--text-color-2);
}

.error-state h2 {
  color: var(--danger-color);
  margin-bottom: 16px;
}

.page-body {
  font-size: 1.1rem;
  line-height: 1.6;
}
</style>
