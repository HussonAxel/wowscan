import {
  createColumnHelper,
  createSortedRowModel,
  rowSortingFeature,
  sortFn_alphanumeric,
  sortFn_basic,
  tableFeatures,
  useTable,
} from '@tanstack/react-table'
import { ArrowDown, ArrowUp } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import { Money } from '@/components/money'
import { Badge } from '@/components/ui/badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { cn } from '@/lib/utils'

export interface RealmPriceRow {
  connectedRealmId: number
  realm: string
  price: number
  auctions: number
  owned: boolean
  best: boolean
}

const features = tableFeatures({
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
  sortFns: { basic: sortFn_basic, alphanumeric: sortFn_alphanumeric },
})
const helper = createColumnHelper<typeof features, RealmPriceRow>()

/** Prix d'un objet par groupe de royaumes, triable, prix alignés à droite. `limit` coupe après le tri. */
export function RealmPriceTable({ rows, limit }: { rows: RealmPriceRow[]; limit?: number }) {
  const { t } = useTranslation()

  const columns = useMemo(
    () =>
      helper.columns([
        helper.accessor('realm', {
          header: t('results.realm'),
          sortFn: 'alphanumeric',
          cell: ({ row }) => (
            <span className="flex items-center gap-2">
              <span className={cn('truncate', row.original.best && 'text-foreground font-medium')}>
                {row.original.realm}
              </span>
              {row.original.owned && (
                <Badge variant="outline" className="border-primary/30 text-primary">
                  {t('results.owned')}
                </Badge>
              )}
            </span>
          ),
        }),
        helper.accessor('auctions', { header: t('results.auctions'), sortFn: 'basic' }),
        helper.accessor('price', {
          header: t('results.price'),
          sortFn: 'basic',
          cell: ({ getValue }) => <Money copper={getValue()} />,
        }),
      ]),
    [t],
  )

  const table = useTable({
    features,
    columns,
    data: rows,
    initialState: { sorting: [{ id: 'price', desc: false }] },
  })

  return (
    <Table>
      <TableHeader className="sticky top-0 z-10 bg-card">
        {table.getHeaderGroups().map((group) => (
          <TableRow key={group.id} className="hover:bg-transparent">
            {group.headers.map((header) => {
              const sorted = header.column.getIsSorted()
              const numeric = header.column.id !== 'realm'
              return (
                <TableHead key={header.id} className={cn(numeric && 'text-right', header.column.id === 'realm' && 'w-full')}>
                  <button
                    type="button"
                    onClick={header.column.getToggleSortingHandler()}
                    className={cn(
                      'inline-flex items-center gap-1 transition-colors duration-150 hover:text-foreground',
                      numeric && 'flex-row-reverse',
                    )}
                  >
                    <table.FlexRender header={header} />
                    {sorted === 'asc' && <ArrowUp className="size-3" />}
                    {sorted === 'desc' && <ArrowDown className="size-3" />}
                  </button>
                </TableHead>
              )
            })}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.slice(0, limit).map((row) => (
          <TableRow key={row.id} className={cn(row.original.best && 'bg-primary/[0.05]')}>
            {row.getAllCells().map((cell) => (
              <TableCell
                key={cell.id}
                className={cn('h-9 py-1.5', cell.column.id !== 'realm' && 'text-right tabular-nums text-muted-foreground', cell.column.id === 'price' && 'text-foreground')}
              >
                <table.FlexRender cell={cell} />
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
