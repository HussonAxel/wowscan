import { createFileRoute } from '@tanstack/react-router'

import { SetsPage } from '@/features/sets/sets-page'

export const Route = createFileRoute('/sets')({ component: SetsPage })
