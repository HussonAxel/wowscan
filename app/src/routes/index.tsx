import { createFileRoute } from '@tanstack/react-router'

import { ScannerPage } from '@/features/scan/scanner-page'

export const Route = createFileRoute('/')({ component: ScannerPage })
