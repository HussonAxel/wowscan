import { createFileRoute } from '@tanstack/react-router'

import { CollectionsPage } from '@/features/collections/collections-page'

export const Route = createFileRoute('/collections')({ component: CollectionsPage })
