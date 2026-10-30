import { useGameIcon } from '@/lib/game-icon'
import { qualityBorder } from '@/lib/format'
import type { ItemInfo } from '@/lib/types'
import { cn } from '@/lib/utils'

/** Icône d'objet cerclée de la couleur de sa qualité ; un carré neutre si elle ne charge pas. */
export function ItemIcon({ item, className }: { item?: ItemInfo; className?: string }) {
  const icons = useGameIcon()
  const src = icons.pick(item?.icon)
  const ring = cn('rounded-md ring-1', qualityBorder[item?.quality ?? ''] ?? 'ring-foreground/10', className)
  if (!src) return <span className={cn('block size-9 bg-foreground/[0.06]', ring)} />
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      onError={() => icons.fail(src)}
      className={cn('size-9 object-cover', ring)}
    />
  )
}
