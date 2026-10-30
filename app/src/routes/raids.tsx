import { createFileRoute } from '@tanstack/react-router'

import { RaidsPage } from '@/features/raids/raids-page'

export const Route = createFileRoute('/raids')({ component: RaidsPage })
