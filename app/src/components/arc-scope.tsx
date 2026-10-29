import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

/**
 * Les composants uiarc lisent leurs propres variables (--accent, --border…),
 * homonymes de celles de shadcn. On ne les définit que dans ce conteneur.
 */
export function ArcScope({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn('arc', className)} data-accent="amber">
      {children}
    </div>
  )
}
