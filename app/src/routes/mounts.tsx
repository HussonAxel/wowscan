import { createFileRoute } from '@tanstack/react-router'

import { MountsPage } from '@/features/mounts/mounts-page'

export const Route = createFileRoute('/mounts')({ component: MountsPage })
